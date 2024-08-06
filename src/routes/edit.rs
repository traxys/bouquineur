use axum::extract::Path;
use base64::prelude::*;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;
use uuid::Uuid;

use crate::{
    metadata::NullableBookDetails,
    models::{BookAuthor, BookComplete, BookTag, User},
    routes::components::book_form,
    schema::{author, book, tag},
    State,
};

use super::{app_page, RouteError};

pub(crate) async fn edit_book(
    state: State,
    user: User,
    id: Path<Uuid>,
) -> Result<maud::Markup, RouteError> {
    let mut conn = state.db.get().await?;

    let book = book::table
        .filter(book::owner.eq(user.id))
        .find(*id)
        .select(BookComplete::as_select())
        .get_result(&mut conn)
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => RouteError::NotFound,
            _ => RouteError::from(e),
        })?;

    let authors = BookAuthor::belonging_to(&book)
        .inner_join(author::table)
        .select(author::name)
        .load::<String>(&mut conn)
        .await?;

    let tags = BookTag::belonging_to(&book)
        .inner_join(tag::table)
        .select(tag::name)
        .load::<String>(&mut conn)
        .await?;

    let image_path = state
        .config
        .metadata
        .image_dir
        .join(user.id.to_string())
        .join(format!("{}.jpg", *id));

    let covert_art_b64 = match image_path.exists() {
        true => Some(BASE64_STANDARD.encode(tokio::fs::read(image_path).await?)),
        false => None,
    };

    let book_details = NullableBookDetails {
        isbn: Some(book.isbn),
        title: Some(book.title),
        authors,
        tags,
        summary: Some(book.summary),
        published: book.published,
        publisher: book.publisher,
        language: book.language,
        google_id: book.googleid,
        amazon_id: book.amazonid,
        librarything_id: book.librarythingid,
        page_count: book.pagecount,
        covert_art_b64,
    };

    Ok(app_page(
        super::Page::Books,
        &user,
        html! {
            (book_form(&state, &user, book_details, "Edit book").await?)
        },
    ))
}
