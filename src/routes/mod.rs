use std::{num::ParseIntError, sync::Arc};

use axum::{
    async_trait,
    extract::{multipart::MultipartError, FromRequestParts},
    http::StatusCode,
    response::IntoResponse,
};
use diesel::prelude::*;
use diesel_async::pooled_connection::deadpool::PoolError;
use diesel_async::RunQueryDsl;
use maud::{html, Markup};

use crate::{
    metadata::MetadataError,
    models::{NewUser, User},
    schema::users,
    AppState, State,
};

mod add;
mod icons;

pub(crate) use add::{add_book, do_add_book};

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
    #[error("Error while handling multipart field")]
    MultipartError(#[from] MultipartError),
    #[error("Invalid date supplied")]
    DateError(#[from] chrono::ParseError),
    #[error("Invalid integer supplied")]
    ParseInt(#[from] ParseIntError),
    #[error("Missing field in form")]
    MissingField,
    #[error("Could not parse image type")]
    ImageDetection(#[source] std::io::Error),
    #[error("Could not parse image")]
    Image(#[from] image::ImageError),
    #[error("Could not save image")]
    ImageSave(#[source] image::ImageError),
    #[error("Invalid fetched image")]
    B64(#[from] base64::DecodeError),
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
            | RouteError::Metadata(_)
            | RouteError::B64(_)
            | RouteError::ImageSave(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Error".into())
            }
            RouteError::InvalidUser(_) => (StatusCode::BAD_REQUEST, "Invalid user name".into()),
            RouteError::MultipartError(e) => (e.status(), e.body_text()),
            RouteError::DateError(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            RouteError::ParseInt(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            RouteError::MissingField => (StatusCode::BAD_REQUEST, "Missing field in form".into()),
            RouteError::ImageDetection(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            RouteError::Image(e) => (StatusCode::BAD_REQUEST, e.to_string()),
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
                link rel="stylesheet"
                     href="https://cdnjs.cloudflare.com/ajax/libs/awesomplete/1.1.7/awesomplete.css"
                     integrity="sha512-GEMEzu9K8wXXaW527IHfGIOaTQ0hXxZPJXZOwGDIO+nrR9Z0ttJih1ZehiEoWY8xPtqzzD7pxAEnQInTZwn3MQ=="
                     crossorigin="anonymous";
                style type="text/css" {
                    (maud::PreEscaped(r#"
                        .awesomplete > ul {
	                        z-index: 10;
                        }
                    "#))
                }
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
                script src="https://cdnjs.cloudflare.com/ajax/libs/awesomplete/1.1.7/awesomplete.min.js"
                       integrity="sha512-Pc3/aEr2FIVZhHxe0RAC9SFrd+pxBJHN3pNJfJNTKc2XAFnXUjgQGIh6X935ePSXNMN6rFa3yftxSnZfJE8ZAg=="
                       crossorigin="anonymous" async {}
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
