use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;
use std::{collections::HashMap, fmt::Write};
use uuid::Uuid;

use crate::{models::User, routes::components, State};

use super::{app_page, series_info, Page, RouteError};

pub(crate) async fn ongoing(state: State, user: User) -> Result<maud::Markup, RouteError> {
    let series = series_info(&state).await?;
    let mut conn = state.db.get().await?;

    let (mut all_owned, mut missing): (Vec<_>, _) = series
        .into_iter()
        .partition(|s| s.total_count.map(|t| t as i64) == Some(s.owned_count));

    all_owned.retain(|s| s.ongoing);
    missing.retain(|s| s.total_count.is_some());

    let mut missing_ids = match missing.is_empty() {
        true => String::new(),
        false => format!("'{}'", missing[0].id),
    };

    if missing.len() > 1 {
        for m in &missing[1..] {
            let _ = write!(missing_ids, ", '{}'", m.id);
        }
    }

    #[derive(QueryableByName, Debug)]
    struct MissingVolume {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        series: Uuid,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        number: i32,
    }

    let mut missing_volumes_table = if missing.is_empty() {
        Default::default()
    } else {
        let missing_books = diesel::sql_query(format!(
            r#"
        SELECT id as series, number 
        FROM series, generate_series(1, total_count) as number 
        WHERE total_count IS NOT NULL
                AND id IN({missing_ids})
        EXCEPT
        SELECT series, number FROM bookseries;
    "#
        ))
        .get_results::<MissingVolume>(&mut conn)
        .await?;

        let mut missing_volumes_table = HashMap::<_, Vec<_>>::new();
        for missing in missing_books {
            missing_volumes_table
                .entry(missing.series)
                .or_default()
                .push(missing.number);
        }

        missing_volumes_table
    };

    missing_volumes_table
        .values_mut()
        .for_each(|v| v.sort_unstable());

    Ok(app_page(
        Page::Ongoing,
        &user,
        html! {
            .container.text-center {
                h2 { "Ongoing Series" }
                @if !missing.is_empty() {
                    h3 { "Missing Volumes" }
                    .ms-3 {
                        @for missing in missing {
                            .col."mb-2" {
                                .card."h-100" style="width: 9.6rem;" {
                                    img src=(components::make_image_url(&state, missing.first_volume, &user)) .card-img-top
                                        alt="first volume cover" style="height: 14.4rem; width: 9.6rem;";
                                    .card-body {
                                        h6 .card-title {
                                            a .nav-link.fs-5 href=(format!("/series/{}", missing.id)) {
                                                (missing.name)
                                            }
                                        }
                                    }
                                    ul .list-group.d-inline-block {
                                        @for v in missing_volumes_table.get(&missing.id).map(|s| -> &[_] { s }).unwrap_or_else(|| &[]) {
                                            li .list-group-item { (format!("Volume {v}")) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                @if !all_owned.is_empty() {
                    h3 { "All Owned" }
                    (components::series_cards(&state, &user, &all_owned))
                }
            }
        },
    ))
}
