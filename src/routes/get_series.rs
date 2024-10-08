use axum::extract::Path;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;
use uuid::Uuid;

use crate::{
    models::{BookPreview, SeriesInfo, User},
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

    let series_info = series::table
        .find(*id)
        .filter(series::owner.eq(user.id))
        .select(SeriesInfo::as_select())
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
                h2 {
                    (series_info.name)
                    @if series_info.ongoing {
                        " (Ongoing)"
                    }
                    a .ms-2.btn.btn-primary href=(format!("{}/edit", *id)) { i .bi.bi-pencil {} }
                }
                (book_cards_for(&state, &user, &series, NO_SORT).await?)
            }
        },
    ))
}
