use axum::extract::Path;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;

use crate::{
    models::{Author, BookAuthor, BookPreview, User},
    routes::book_cards_for,
    schema::{author, book},
    State,
};

use super::{app_page, RouteError};

pub(crate) async fn get_author(
    state: State,
    user: User,
    id: Path<i32>,
) -> Result<maud::Markup, RouteError> {
    let mut conn = state.db.get().await?;

    let author_info = author::table
        .find(*id)
        .select(Author::as_select())
        .get_result(&mut conn)
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => RouteError::NotFound,
            _ => e.into(),
        })?;

    let author_books: Vec<BookPreview> = BookAuthor::belonging_to(&author_info)
        .inner_join(book::table)
        .filter(book::owner.eq(user.id))
        .select(BookPreview::as_select())
        .get_results(&mut conn)
        .await?;

    // Because we perform more work to get here author ids can be guessed, but not more
    if author_books.is_empty() {
        return Err(RouteError::NotFound);
    }

    let date_sort = |a: &BookPreview, b: &BookPreview| match (a.published, b.published) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, _) | (_, None) => std::cmp::Ordering::Less,
        (Some(a), Some(b)) => a.cmp(&b),
    };

    Ok(app_page(
        super::Page::Books,
        &user,
        html! {
            .text-center {
                h2 { (author_info.name) }
                (book_cards_for(&state, &user, &author_books, Some(date_sort)).await?)
            }
        },
    ))
}
