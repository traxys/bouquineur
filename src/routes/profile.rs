use maud::html;

use super::{raw_app_page, RouteError, State, User};

pub(crate) async fn profile(state: State, user: User) -> Result<maud::Markup, RouteError> {
    Ok(raw_app_page(
        None,
        &user,
        html! {
            .text-center {
                h2 { (format!("Profile for {}", user.name)) }
            }
        },
    ))
}
