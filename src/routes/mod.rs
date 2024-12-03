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
use diesel::{prelude::*, sql_types};
use diesel_async::pooled_connection::deadpool::PoolError;
use diesel_async::RunQueryDsl;
use maud::{html, Markup};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::{
    metadata::MetadataError,
    models::{AuthorName, Book, BookPreview, NewUser, TagName, User},
    schema::{book, bookseries, users},
    AppState, State,
};

mod add;
mod edit;
mod edit_series;
mod get_author;
mod get_book;
mod get_series;
mod icons;
mod ongoing;
mod profile;
mod unread;

mod components;

pub(crate) use add::{add_book, do_add_book};
pub(crate) use edit::{do_edit_book, edit_book};
pub(crate) use edit_series::{do_series_edit, series_edit};
pub(crate) use get_author::get_author;
pub(crate) use get_book::get_book;
pub(crate) use get_series::get_series;
pub(crate) use ongoing::{ongoing, ongoing_public};
pub(crate) use profile::{do_edit_profile, profile};
pub(crate) use unread::unread;

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

#[derive(serde::Serialize, serde::Deserialize)]
enum CheckboxTick {
    #[serde(rename = "on")]
    On,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Page {
    Books,
    Series,
    AddBook,
    Unread,
    Ongoing,
}

impl Page {
    fn variants() -> &'static [Self] {
        &[
            Self::Books,
            Self::Unread,
            Self::Series,
            Self::Ongoing,
            Self::AddBook,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Page::Books => "Books",
            Page::Unread => "Unread",
            Page::Series => "Series",
            Page::AddBook => "Add a Book",
            Page::Ongoing => "Ongoing",
        }
    }

    pub fn location(&self) -> &'static str {
        match self {
            Page::Books => "/",
            Page::Unread => "/unread",
            Page::AddBook => "/add",
            Page::Series => "/series",
            Page::Ongoing => "/ongoing",
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
                script {
                    (maud::PreEscaped(r#"
                        const tooltipTriggerList = document.querySelectorAll('[data-bs-toggle="tooltip"]')
                        const tooltipList = [...tooltipTriggerList].map(tooltipTriggerEl => new bootstrap.Tooltip(tooltipTriggerEl))
                    "#))
                }
            }
        }
    }
}

fn base_page(body: Markup) -> Markup {
    base_page_with_head(body, None)
}

fn raw_app_page(page: Option<Page>, user: &User, body: Markup) -> Markup {
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
                        @let current = Some(*p) == page;
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
                    a href="/profile" .align-middle.link-light { (user.name) }
                }
            }
            (body)
        }
    })
}

fn app_page(page: Page, user: &User, body: Markup) -> Markup {
    raw_app_page(Some(page), user, body)
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
    series: Option<(String, i32)>,
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
            series_name: Option<String>,
            series_volume: Option<i32>,
            owned_box: bool,
            read_box: bool,
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
                "series_name" => data.series_name = load(field.text().await?),
                "series_volume" => {
                    let text = field.text().await?;
                    if !text.is_empty() {
                        data.series_volume = Some(text.parse()?);
                    }
                }
                "owned_box" => data.owned_box = true,
                "read_box" => data.read_box = true,
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
            owned: data.owned_box,
            read: data.read_box,
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

        let series = match (data.series_name, data.series_volume) {
            (None, None) => None,
            (Some(name), Some(volume)) => Some((name, volume)),
            _ => return Err(RouteError::MissingField),
        };

        Ok(BookInfo {
            book,
            image,
            series,
            authors: data.authors,
            tags: data.tags,
        })
    }
}

pub(crate) async fn image(
    state: State,
    Path((user_id, book_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, RouteError> {
    let image_path = state
        .config
        .metadata
        .image_dir
        .join(user_id.to_string())
        .join(format!("{}.jpg", book_id));

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
        .left_join(bookseries::table)
        .order((bookseries::series, bookseries::number, book::title))
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

#[derive(QueryableByName)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SeriesAllInfo {
    #[diesel(sql_type = sql_types::Uuid)]
    pub id: Uuid,
    #[diesel(sql_type = sql_types::VarChar)]
    pub name: String,
    #[diesel(sql_type = sql_types::Bool)]
    pub ongoing: bool,
    #[diesel(sql_type = sql_types::BigInt)]
    pub owned_count: i64,
    #[diesel(sql_type = sql_types::Uuid)]
    pub first_volume: Uuid,
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    pub total_count: Option<i32>,
}

async fn series_info(state: &State) -> Result<Vec<SeriesAllInfo>, RouteError> {
    let mut conn = state.db.get().await?;

    let series = diesel::sql_query(
        r#"
        SELECT 
            bs.book as first_volume,
            bs.series as id,
            series.name as name,
            ongoing,
            total_count,
            COALESCE(owned_count, 0) as owned_count
        FROM 
            bookseries bs 
        INNER JOIN 
            (SELECT series, min(number) as minvolume FROM bookseries GROUP BY series) b 
            ON b.series = bs.series AND bs.number = b.minvolume 
        INNER JOIN 
            series 
            ON series.id = bs.series
        LEFT JOIN
            (
                SELECT series, COUNT(book) as owned_count
                FROM bookseries 
                INNER JOIN book ON book.id = bookseries.book AND book.owned
                GROUP BY series
            ) as owned_book_count
            ON owned_book_count.series = bs.series;
    "#,
    )
    .get_results::<SeriesAllInfo>(&mut conn)
    .await?;

    Ok(series)
}

pub(crate) async fn series(state: State, user: User) -> Result<maud::Markup, RouteError> {
    let series = series_info(&state).await?;

    Ok(app_page(
        Page::Series,
        &user,
        html! {
            .text-center {
                h2 { "Series" }
                (components::series_cards(&state, &user, &series, true))
            }
        },
    ))
}
