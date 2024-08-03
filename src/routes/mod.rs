use std::sync::{Arc, LazyLock};

use axum::{
    async_trait,
    extract::{FromRequestParts, Query},
    http::StatusCode,
    response::IntoResponse,
};
use base64::prelude::*;
use diesel::prelude::*;
use diesel_async::pooled_connection::deadpool::PoolError;
use diesel_async::RunQueryDsl;
use maud::{html, Markup};

use crate::{
    metadata::{fetch_metadata, MetadataError, NullableBookDetails},
    models::{NewUser, User},
    schema::users,
    AppState, State,
};

mod icons;

#[derive(thiserror::Error, Debug)]
pub(crate) enum RouteError {
    #[error("Database error")]
    Db(#[from] diesel::result::Error),
    #[error("Missing a user header")]
    NoUser,
    #[error("Could not parse user name")]
    InvalidUser(#[from] axum::http::header::ToStrError),
    #[error("Could not get a connection from the pool")]
    PoolError(#[from] PoolError),
    #[error("Could not fetch metadata")]
    Metadata(#[from] MetadataError),
}

impl IntoResponse for RouteError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!("route error: {self} ({self:#?})");
        let (code, text) = match self {
            // Don't reveal the missing authenitication header to the client, this is a
            // mis-configuration that could be exploited
            RouteError::Db(_)
            | RouteError::NoUser
            | RouteError::PoolError(_)
            | RouteError::Metadata(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Error"),
            RouteError::InvalidUser(_) => (StatusCode::BAD_REQUEST, "Invalid user name"),
        };

        (
            code,
            base_page(html! {
                h1 { "Fatal Error encountered" }
                p { (text) }
            }),
        )
            .into_response()
    }
}

#[derive(PartialEq, Eq)]
enum Page {
    Books,
    AddBook,
}

impl Page {
    fn variants() -> &'static [Self] {
        &[Self::Books, Self::AddBook]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Page::Books => "Books",
            Page::AddBook => "Add a Book",
        }
    }

    pub fn location(&self) -> &'static str {
        match self {
            Page::Books => "/",
            Page::AddBook => "/add",
        }
    }
}

fn base_page_with_head(body: Markup, head: Option<Markup>) -> Markup {
    html! {
        (maud::DOCTYPE)
        html lang="en" data-bs-theme="dark" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Bouquineur" }
                link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/css/bootstrap.min.css"
                     rel="stylesheet"
                     integrity="sha384-T3c6CoIi6uLrA9TneNEoa7RxnatzjcDSCmG1MXxSR1GAsXEV/Dwwykc2MPK8M2HN"
                     crossorigin="anonymous";
                link rel="stylesheet"
                     href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.3/font/bootstrap-icons.min.css"
                     integrity="sha384-XGjxtQfXaH2tnPFa9x+ruJTuLE3Aa6LhHSWRr1XeTyhezb4abCG4ccI5AkVDxqC+"
                     crossorigin="anonymous";
                @if let Some(head) = head {
                    (head)
                }
            }
            body {
                (body)
                script src="https://cdn.jsdelivr.net/npm/@undecaf/zbar-wasm@0.9.15/dist/index.js"
                       integrity="sha384-yW9Y7lGkfKYN+jnhSQpcumEsBkSCx/Ab9s2+rHyU5faxR81n4c2mhBw1K6TyFG2a"
                       crossorigin="anonymous" {}
                script src="https://cdn.jsdelivr.net/npm/@undecaf/barcode-detector-polyfill@0.9.21/dist/index.js"
                       integrity="sha384-MOAlrmENITvPLnTzISP6k/GAbCgTOuREHSbC1X5a3qcIHeHTNilNuzc7LfXVYKMO"
                       crossorigin="anonymous" {}
                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/js/bootstrap.bundle.min.js"
                       integrity="sha384-C6RzsynM9kWDrMNeT87bh95OGNyZPhcTNXj1NW7RuBCsyN/o0jlpcV8Qyq46cDfL"
                       crossorigin="anonymous" {}
                script src="https://unpkg.com/htmx.org@2.0.1"
                       integrity="sha384-QWGpdj554B4ETpJJC9z+ZHJcA/i59TyjxEPXiiUgN2WmTyV5OEZWCD6gQhgkdpB/"
                       crossorigin="anonymous" {}
            }
        }
    }
}

fn base_page(body: Markup) -> Markup {
    base_page_with_head(body, None)
}

fn app_page(page: Page, user: &User, body: Markup) -> Markup {
    base_page(html! {
        .container-fluid {
            header .d-flex
                   .flex-wrap
                   .align-items-center
                   .justify-content-center
                   .justify-content-md-between
                   ."py-3"."mb-4" {
                h2 ."col-md-3"."mb-2"."mb-md-0" {
                    a .d-inline-flex.link-body-emphasis.text-decoration-none href="/" {
                        i .bi.bi-book-half {}
                    }
                }
                ul .nav.nav-pills."col-12".col-md-auto."mb-2".justify-content-center."mb-md-0" {
                    @for p in Page::variants() {
                        @let current = *p == page;
                        li .nav-item {
                            a .nav-link.active[current]
                                aria-current=[current.then(|| "page")]
                                href=(p.location()) {
                                (p.name())
                            }
                        }
                    }
                }
                ."col-md-3".text-end."me-2" {
                    span .align-middle { (user.name) }
                }
            }
            (body)
        }
    })
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for User {
    type Rejection = RouteError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let user = match parts.headers.get(&state.config.auth.header) {
            Some(user) => user.to_str()?,
            None if state.config.debug.assume_user.is_some() => {
                state.config.debug.assume_user.as_deref().unwrap()
            }
            None => {
                return Err(RouteError::NoUser);
            }
        };

        let mut conn = state.db.get().await?;

        diesel::insert_into(users::table)
            .values(&NewUser { name: user })
            .on_conflict_do_nothing()
            .execute(&mut conn)
            .await?;

        Ok(users::table
            .filter(users::name.eq(user))
            .select(User::as_select())
            .first(&mut conn)
            .await?)
    }
}

pub(crate) async fn index(state: State, user: User) -> maud::Markup {
    app_page(
        Page::Books,
        &user,
        html! {
            p { "Hello, " (user.name) "!" }
        },
    )
}

static NO_COVER: LazyLock<String> = LazyLock::new(|| {
    let image = include_bytes!("../no_cover.jpg");
    BASE64_STANDARD.encode(image)
});

fn book_form(details: NullableBookDetails) -> maud::Markup {
    let image = details
        .covert_art_b64
        .as_ref()
        .unwrap_or_else(|| &*NO_COVER);

    html! { form .container-sm.align-items-center method="POST" .mt-2 {
        .text-center.d-flex.flex-column."mb-2" {
            label for="coverArtInput" .form-label {"Cover art"}
            div {
                img .img-fluid."mb-2"
                    #coverArt
                    style="height:400px;"
                    alt="Cover Art"
                    src=(format!("data:image/jpg;base64,{image}"));
            }
            input .form-control accept="image/*" type="file" name="user_cover" #coverArtInput;
            script {
                (maud::PreEscaped(r#"
                    coverArt = document.getElementById("coverArt")
                    coverArtInput = document.getElementById("coverArtInput")
            
                    coverArtInput.onchange = evt => {
                        const [file] = coverArtInput.files
                        if (file) {
                            coverArt.src = URL.createObjectURL(file)
                        }
                    }
                "#))
            }
            @if let Some(b64) = details.covert_art_b64 {
                input type="hidden" value=(b64) name="fetched_cover";
            }
        }
        .form-floating."mb-2" {
            input .form-control required #title name="title" type="text" placeholder="Title" value=[details.title];
            label for="title" { "Title" }
        }
        .form-floating."mb-2" {
            input .form-control required #isbn name="isbn" type="text" placeholder="ISBN" value=[details.isbn];
            label for="isbn" { "ISBN" }
        }
        .form-floating."mb-2" {
            textarea .form-control placeholder="Book summary" #summary style="height: 150px" {
                (details.summary.unwrap_or_default())
            }
            label for="summary" { "Summary" }
        }
        .form-floating."mb-2" {
            input #published name="published" type="date" .form-control placeholder="1970-01-01"
                  value=[details.published.map(|d| d.format("%Y-%m-%d"))];
            label for="published" {"Publication Date"}
        }
        .form-floating."mb-2" {
            input .form-control #publisher name="publisher" type="text" placeholder="Publisher" value=[details.publisher];
            label for="publisher" { "Publisher" }
        }
        .form-floating."mb-2" {
            input .form-control #language name="language" type="text" placeholder="Language" value=[details.language];
            label for="language" { "Language" }
        }
        .form-floating."mb-2" {
            input .form-control #googleID name="google_id" type="text" placeholder="Google ID" value=[details.google_id];
            label for="googleID" { "Google ID" }
        }
        .form-floating."mb-2" {
            input .form-control #amazonID name="amazon_id" type="text" placeholder="Amazon ID" value=[details.amazon_id];
            label for="amazonID" { "Amazon ID" }
        }
        .form-floating."mb-2" {
            input .form-control #librarythingId name="librarything_id" type="text" placeholder="Librarything ID" value=[details.librarything_id];
            label for="librarythingId" { "Librarything ID" }
        }
        .form-floating."mb-2" {
            input .form-control #pageCount name="page_count" type="number" placeholder="Page Count" value=[details.page_count];
            label for="pageCount" { "Page Count" }
        }
        input type="submit" .btn.btn-primary value="Add Book";
    } }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IsbnRequest {
    isbn: Option<String>,
}

pub(crate) async fn add_book(
    state: State,
    user: User,
    isbn: Query<IsbnRequest>,
) -> Result<maud::Markup, RouteError> {
    let (not_found, book_details) = match &isbn.isbn {
        None => (false, (NullableBookDetails::default())),
        Some(isbn) => fetch_metadata(&state.config, isbn)
            .await?
            .map(|v| (false, v))
            .unwrap_or_else(|| (true, Default::default())),
    };

    Ok(app_page(
        Page::AddBook,
        &user,
        html! {
            #isbnModal .modal.fade tabindex="-1" aria-labelledby="isbnModalLabel" aria-hidden="true" {
                .modal-dialog.modal-dialog-centered { .modal-content {
                    .modal-header {
                        h1 .modal-title."fs-5" #isbnModalLabel {"Load a book from an ISBN"}
                        button type="button" .btn-close data-bs-dismiss="modal" aria-label="Cancel" {}
                    }
                    .modal-body {
                        form #isbnModalForm {
                            .form-floating {
                                input name="isbn"
                                        type="text"
                                        .form-control
                                        #isbnSearch
                                        placeholder="978-3-16-148410-0";
                                label for="isbnSearch" { "ISBN" }
                            }
                        }
                    }
                    .modal-footer {
                        button type="button" .btn.btn-secondary data-bs-dismiss="modal" { "Cancel" }
                        button type="submit" form="isbnModalForm" .btn.btn-primary { "Load" }
                    }
                }  }
            }

            #scanModal .modal.fade tabindex="-1" aria-labelledby="scanModalLabel" aria-hidden="true" {
                .modal-dialog.modal-dialog-centered { .modal-content {
                    .modal-header {
                        h1 .modal-title."fs-5" #scanModalLabel {"Load a book from an ISBN barcode"}
                        button type="button" .btn-close data-bs-dismiss="modal" aria-label="Cancel" {}
                    }
                    .modal-body {
                        video #scanVideo width="300" height="200" style="border: 1px solid gray" {}
                    }
                    .modal-footer {
                        button type="button" .btn.btn-secondary data-bs-dismiss="modal" { "Cancel" }
                    }
                }  }
            }

            @if not_found {
                .alert.alert-warning role="alert" {
                    "The requested ISBN was not found"
                }
            }

            .d-flex.flex-column {
                .d-flex.justify-content-center {
                    button .btn.btn-primary.me-2 data-bs-toggle="modal" data-bs-target="#isbnModal" {
                        (icons::bi_123()) "Load from ISBN"
                    }
                    button .btn.btn-primary data-bs-toggle="modal" data-bs-target="#scanModal" {
                        (icons::bi_upc_scan()) "Scan ISBN"
                    }
                }
                (book_form(&state, &user, book_details).await?)
            }

            script {
                (maud::PreEscaped(include_str!("./barcode.js")))
            }
        },
    ))
}
