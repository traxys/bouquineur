use axum::{extract::Path, Form};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;
use uuid::Uuid;

use crate::{
    models::{SeriesInfo, User},
    schema::series,
    State,
};

use super::{app_page, RouteError};

fn empty_string_as_none<'de, D>(de: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptI32Visitor;
    impl<'de> serde::de::Visitor<'de> for OptI32Visitor {
        type Value = Option<i32>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "an empty string or integer")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            match v {
                "" => Ok(None),
                v => v.parse().map_err(E::custom).map(Some),
            }
        }
    }

    de.deserialize_any(OptI32Visitor)
}

#[derive(serde::Deserialize)]
pub(crate) struct SeriesForm {
    name: String,
    ongoing_box: Option<super::CheckboxTick>,
    #[serde(deserialize_with = "empty_string_as_none")]
    total_count: Option<i32>,
}

impl SeriesForm {
    fn changeset(self) -> SeriesEdit {
        SeriesEdit {
            name: self.name,
            total_count: self.total_count,
            ongoing: self.ongoing_box.is_some(),
        }
    }
}

#[derive(diesel::AsChangeset)]
#[diesel(table_name = crate::schema::series)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct SeriesEdit {
    name: String,
    ongoing: bool,
    #[diesel(treat_none_as_null = true)]
    total_count: Option<i32>,
}

pub(crate) async fn do_series_edit(
    state: State,
    user: User,
    id: Path<Uuid>,
    Form(form): Form<SeriesForm>,
) -> Result<axum::response::Redirect, RouteError> {
    let mut conn = state.db.get().await?;

    diesel::update(series::table)
        .filter(series::id.eq(*id).and(series::owner.eq(user.id)))
        .set(form.changeset())
        .execute(&mut conn)
        .await?;

    Ok(axum::response::Redirect::to(&format!("/series/{}", *id)))
}

pub(crate) async fn series_edit(
    state: State,
    user: User,
    id: Path<Uuid>,
) -> Result<maud::Markup, RouteError> {
    let mut conn = state.db.get().await?;

    let s = series::table
        .find(*id)
        .filter(series::owner.eq(user.id))
        .select(SeriesInfo::as_select())
        .get_result(&mut conn)
        .await
        .map_err(|e| match e {
            diesel::result::Error::NotFound => RouteError::NotFound,
            _ => e.into(),
        })?;

    Ok(app_page(
        super::Page::Series,
        &user,
        html! {
            form .container-sm.align-items-center method="POST" {
                .container.text-center {
                    h1 { "Edit Series" }
                }
                .form-floating.mb-2 {
                    input .form-control required #name name="name" type="text" placeholder="Name"
                        value=(s.name);
                    label for="name" { "Name" }
                }
                .form-check {
                    input .form-check-input type="checkbox" name="ongoing_box" #ongoingBox checked[s.ongoing];
                    label .form-check-label for="ongoingBox" { "Ongoing" }
                }
                .form-floating."mb-2" {
                    input .form-control required #totalCount name="total_count" type="number"
                            placeholder="Total Count" value=[s.total_count];
                    label for="totalCount" { "Total Count" }
                }
                .container.text-center {
                    input  type="submit" .btn.btn-primary value="Edit series";
                }
            }
        },
    ))
}
