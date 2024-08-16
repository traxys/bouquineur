use std::collections::HashMap;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;

use crate::{
    models::{BookPreview, SeriesInfo, User},
    routes::components::{book_cards_for, NO_SORT},
    schema::{book, bookseries, series},
    State,
};

use super::{app_page, RouteError};

pub(crate) async fn unread(state: State, user: User) -> Result<maud::Markup, RouteError> {
    let mut conn = state.db.get().await?;

    let unread: Vec<(BookPreview, Option<SeriesInfo>)> = book::table
        .filter(book::read.eq(false).and(book::owner.eq(user.id)))
        .left_join(bookseries::table.inner_join(series::table))
        .select((BookPreview::as_select(), Option::<SeriesInfo>::as_select()))
        .load(&mut conn)
        .await?;

    let mut by_series = HashMap::new();

    for (book, series) in unread {
        by_series.entry(series).or_insert_with(Vec::new).push(book);
    }

    let no_series = by_series.remove(&None).unwrap_or_default();

    Ok(app_page(
        super::Page::Unread,
        &user,
        html! { .container {
            (book_cards_for(&state, &user, &no_series, NO_SORT).await?)
            @for (s, books) in by_series {
                h2 { (s.unwrap().name) }
                (book_cards_for(&state, &user, &books, NO_SORT).await?)
            }
        }},
    ))
}
