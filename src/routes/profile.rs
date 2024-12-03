use axum::{response::Redirect, Form};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::html;

use crate::schema::users;

use super::{raw_app_page, RouteError, State, User};

#[derive(diesel::AsChangeset, diesel::Selectable, diesel::Queryable)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct ProfileEdit {
    public_ongoing: bool,
}

#[derive(serde::Deserialize)]
pub(crate) struct ProfileForm {
    ongoing_box: Option<super::CheckboxTick>,
}

pub(crate) async fn do_edit_profile(
    state: State,
    user: User,
    Form(form): Form<ProfileForm>,
) -> Result<Redirect, RouteError> {
    let mut conn = state.db.get().await?;

    diesel::update(users::table)
        .filter(users::id.eq(user.id))
        .set(ProfileEdit {
            public_ongoing: form.ongoing_box.is_some(),
        })
        .execute(&mut conn)
        .await?;

    Ok(axum::response::Redirect::to("/profile"))
}

pub(crate) async fn profile(state: State, user: User) -> Result<maud::Markup, RouteError> {
    let mut conn = state.db.get().await?;

    let profile = users::table
        .find(user.id)
        .select(ProfileEdit::as_select())
        .get_result(&mut conn)
        .await?;

    let public_url = format!("/public/{}/ongoing", user.id);

    Ok(raw_app_page(
        None,
        &user,
        html! {
            form .container-sm.align-items-center method="POST" {
                .container.text-center {
                    h1 { (format!("Profile for {}", user.name)) }
                }
                .form-check {
                    input .form-check-input type="checkbox" name="ongoing_box" #ongoingBox checked[profile.public_ongoing];
                    label .form-check-label for="ongoingBox" { "Public Ongoing" }
                    @if profile.public_ongoing {
                        " " a href=(public_url) {"(Public URL)"}
                    }
                }
                .container.text-center {
                    input  type="submit" .btn.btn-primary value="Edit profile";
                }
            }
        },
    ))
}
