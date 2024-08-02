use std::sync::Arc;

use axum::{async_trait, extract::FromRequestParts, http::StatusCode, response::IntoResponse};
use diesel::prelude::*;
use diesel_async::pooled_connection::deadpool::PoolError;
use diesel_async::RunQueryDsl;
use maud::{html, Markup};

use crate::{
    models::{NewUser, User},
    schema::users,
    AppState, State,
};

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
}

impl IntoResponse for RouteError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!("route error: {self:#?}");
        let (code, text) = match self {
            // Don't reveal the missing authenitication header to the client, this is a
            // mis-configuration that could be exploited
            RouteError::Db(_) | RouteError::NoUser | RouteError::PoolError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Error")
            }
            RouteError::InvalidUser(_) => (StatusCode::BAD_REQUEST, "Invalid user name"),
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

#[derive(PartialEq, Eq)]
enum Page {
    Books,
    AddBook,
}

impl Page {
    fn variants() -> &'static [Self] {
        &[Self::Books, Self::AddBook]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Page::Books => "Books",
            Page::AddBook => "Add a Book",
        }
    }

    pub fn location(&self) -> &'static str {
        match self {
            Page::Books => "/",
            Page::AddBook => "/add",
        }
    }
}

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
                @if let Some(head) = head {
                    (head)
                }
            }
            body {
                (body)
                script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/js/bootstrap.bundle.min.js"
                       integrity="sha384-C6RzsynM9kWDrMNeT87bh95OGNyZPhcTNXj1NW7RuBCsyN/o0jlpcV8Qyq46cDfL"
                       crossorigin="anonymous" {}
            }
        }
    }
}

fn base_page(body: Markup) -> Markup {
    base_page_with_head(body, None)
}

fn app_page(body: Markup, page: Page) -> Markup {
    base_page(html! {
        .container-fluid {
            header .d-flex.justify-content-center."py-3" {
                ul .nav.nav-pills {
                    @for p in Page::variants() {
                        @let current = *p == page;
                        li .nav-item {
                            a .nav-link.active[current]
                                aria-current=[current.then(|| "page")]
                                href=(p.location()) {
                                (p.name())
                            }
                        }
                    }
                }
            }
            (body)
        }
    })
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

pub(crate) async fn index(state: State, user: User) -> maud::Markup {
    app_page(
        html! {
            p { "Hello!" }
        },
        Page::Books,
    )
}
