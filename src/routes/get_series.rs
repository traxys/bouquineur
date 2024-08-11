use axum::extract::Path;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;
use uuid::Uuid;

use crate::{
    models::{BookPreview, User},
    routes::components::{book_cards_for, NO_SORT},
    schema::{book, bookseries, series},
    State,
};

use super::{app_page, RouteError};

pub(crate) async fn get_series(
    state: State,
    user: User,
    id: Path<Uuid>,
) -> Result<maud::Markup, RouteError> {
    let mut conn = state.db.get().await?;

    let series_info: String = series::table
        .find(*id)
        .filter(series::owner.eq(user.id))
        .select(series::name)
        .get_result(&mut conn)
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => RouteError::NotFound,
            _ => e.into(),
        })?;

    let series = bookseries::table
        .inner_join(book::table)
        .filter(bookseries::series.eq(*id))
        .filter(book::owner.eq(user.id))
        .select(BookPreview::as_select())
        .order(bookseries::number.asc())
        .get_results(&mut conn)
        .await?;

    Ok(app_page(
        super::Page::Series,
        &user,
        html! {
            .text-center {
                h2 { (series_info) }
                (book_cards_for(&state, &user, &series, NO_SORT).await?)
            }
        },
    ))
}
