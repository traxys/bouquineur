use std::{
    io::Cursor,
    num::ParseIntError,
    sync::{Arc, LazyLock},
};

use axum::{
    async_trait,
    body::{Body, Bytes},
    extract::{
        multipart::{MultipartError, MultipartRejection},
        FromRequest, FromRequestParts, Multipart, Path, Request,
    },
    http::{header::CONTENT_TYPE, StatusCode},
    response::IntoResponse,
    RequestExt,
};
use base64::prelude::*;
use chrono::NaiveDate;
use components::{book_cards_for, NO_SORT};
use diesel::prelude::*;
use diesel_async::pooled_connection::deadpool::PoolError;
use diesel_async::RunQueryDsl;
use maud::{html, Markup};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::{
    metadata::MetadataError,
    models::{AuthorName, Book, BookPreview, NewUser, TagName, User},
    schema::{book, users},
    AppState, State,
};

mod add;
mod edit;
mod get_author;
mod get_book;
mod icons;

mod components;

pub(crate) use add::{add_book, do_add_book};
pub(crate) use edit::{do_edit_book, edit_book};
pub(crate) use get_author::get_author;
pub(crate) use get_book::get_book;

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
    #[error("Resource not found")]
    NotFound,
    #[error("Unexpected IO error")]
    IO(#[from] std::io::Error),
    #[error("Invalid multipart")]
    Multipart(#[from] MultipartRejection),
}

impl IntoResponse for RouteError {
    fn into_response(self) -> axum::response::Response {
        if !matches!(&self, Self::MultipartError(_)) {
            tracing::error!("route error: {self} ({self:#?})");
        }

        let (code, text) = match self {
            // Don't reveal the missing authenitication header to the client, this is a
            // mis-configuration that could be exploited
            RouteError::Db(_)
            | RouteError::NoUser
            | RouteError::PoolError(_)
            | RouteError::Metadata(_)
            | RouteError::B64(_)
            | RouteError::ImageSave(_)
            | RouteError::IO(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Error".into()),
            RouteError::InvalidUser(_) => (StatusCode::BAD_REQUEST, "Invalid user name".into()),
            RouteError::MultipartError(e) => (e.status(), e.body_text()),
            RouteError::DateError(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            RouteError::ParseInt(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            RouteError::MissingField => (StatusCode::BAD_REQUEST, "Missing field in form".into()),
            RouteError::ImageDetection(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            RouteError::Image(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            RouteError::NotFound => (StatusCode::NOT_FOUND, "Resource not found".into()),
            RouteError::Multipart(r) => return r.into_response(),
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

static NO_COVER: LazyLock<String> = LazyLock::new(|| {
    let image = include_bytes!("../no_cover.jpg");
    BASE64_STANDARD.encode(image)
});

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

#[derive(Debug)]
pub(crate) struct BookInfo {
    book: Book,
    image: Option<image::DynamicImage>,
    authors: Vec<AuthorName>,
    tags: Vec<TagName>,
}

#[async_trait]
impl FromRequest<Arc<AppState>> for BookInfo {
    type Rejection = RouteError;

    async fn from_request(
        mut req: Request,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let user: User = req.extract_parts_with_state(state).await?;
        let mut multipart = Multipart::from_request(req, state).await?;

        enum CoverArt {
            User(Bytes),
            Fetched(String),
        }

        #[derive(Default)]
        struct BookData {
            cover_art: Option<CoverArt>,
            title: Option<String>,
            isbn: Option<String>,
            summary: String,
            authors: Vec<AuthorName>,
            tags: Vec<TagName>,
            publication_date: Option<NaiveDate>,
            publisher: Option<String>,
            language: Option<String>,
            google_id: Option<String>,
            amazon_id: Option<String>,
            librarything_id: Option<String>,
            page_count: Option<i32>,
        }

        let mut data = BookData::default();
        let load = |s: String| if s.is_empty() { None } else { Some(s) };

        while let Some(field) = multipart.next_field().await? {
            let Some(name) = field.name() else {
                tracing::warn!("Unamed multipart field");
                continue;
            };

            match name {
                "user_cover" => {
                    let cover = field.bytes().await?;
                    if !cover.is_empty() {
                        data.cover_art = Some(CoverArt::User(cover));
                    }
                }
                "fetched_cover" => {
                    if data.cover_art.is_none() {
                        data.cover_art = Some(CoverArt::Fetched(field.text().await?));
                    }
                }
                "title" => data.title = load(field.text().await?),
                "isbn" => data.isbn = load(field.text().await?),
                "summary" => data.summary = field.text().await?,
                "author" => data.authors.push(AuthorName {
                    name: field.text().await?,
                }),
                "tag" => data.tags.push(TagName {
                    name: field.text().await?,
                }),
                "published" => {
                    let text = field.text().await?;
                    if !text.is_empty() {
                        data.publication_date = Some(NaiveDate::parse_from_str(&text, "%Y-%m-%d")?)
                    }
                }
                "publisher" => data.publisher = load(field.text().await?),
                "language" => data.language = load(field.text().await?),
                "google_id" => data.google_id = load(field.text().await?),
                "amazon_id" => data.amazon_id = load(field.text().await?),
                "librarything_id" => data.librarything_id = load(field.text().await?),
                "page_count" => {
                    let text = field.text().await?;
                    if !text.is_empty() {
                        data.page_count = Some(text.parse()?)
                    }
                }
                _ => {
                    tracing::warn!("Unknown field {:?}", field.name());
                }
            }
        }

        let book = Book {
            owner: user.id,
            isbn: data.isbn.ok_or(RouteError::MissingField)?,
            title: data.title.ok_or(RouteError::MissingField)?,
            summary: data.summary,
            published: data.publication_date,
            publisher: data.publisher,
            language: data.language,
            googleid: data.google_id,
            amazonid: data.amazon_id,
            librarythingid: data.librarything_id,
            pagecount: data.page_count,
        };

        let image = match data.cover_art {
            Some(CoverArt::User(bytes)) => Some(
                image::ImageReader::new(Cursor::new(bytes))
                    .with_guessed_format()
                    .map_err(RouteError::ImageDetection)?
                    .decode()?,
            ),
            Some(CoverArt::Fetched(data)) => {
                let data = BASE64_STANDARD.decode(data)?;

                Some(
                    image::ImageReader::new(Cursor::new(data))
                        .with_guessed_format()
                        .map_err(RouteError::ImageDetection)?
                        .decode()?,
                )
            }
            None => None,
        };

        Ok(BookInfo {
            book,
            image,
            authors: data.authors,
            tags: data.tags,
        })
    }
}

pub(crate) async fn image(
    state: State,
    user: User,
    book_id: Path<Uuid>,
) -> Result<impl IntoResponse, RouteError> {
    let image_path = state
        .config
        .metadata
        .image_dir
        .join(user.id.to_string())
        .join(format!("{}.jpg", *book_id));

    if !image_path.exists() {
        return Err(RouteError::NotFound);
    }

    let file = tokio::fs::File::open(image_path).await?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok(([(CONTENT_TYPE, "image/jpeg")], body).into_response())
}

pub(crate) async fn image_not_found(_user: User) -> impl IntoResponse {
    let image = include_bytes!("../no_cover.jpg");

    ([(CONTENT_TYPE, "image/jpeg")], image)
}

pub(crate) async fn index(state: State, user: User) -> Result<maud::Markup, RouteError> {
    let mut conn = state.db.get().await?;

    let all_books: Vec<BookPreview> = book::table
        .filter(book::owner.eq(user.id))
        .select(BookPreview::as_select())
        .load(&mut conn)
        .await?;

    drop(conn);

    let book_data = book_cards_for(&state, &user, &all_books, NO_SORT).await?;

    Ok(app_page(
        Page::Books,
        &user,
        html! {
            .text-center {
                h2 { "Books" }
                (book_data)
            }
        },
    ))
}
