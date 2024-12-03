use axum::extract::Path;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::{html, PreEscaped};
use uuid::Uuid;

use crate::{
    models::{Author, BookAuthor, BookComplete, BookTag, User},
    schema::{author, book, bookseries, series, tag},
    State,
};

use super::{app_page, RouteError};

pub(crate) async fn get_book(
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

    let series: Option<(String, i32, Uuid)> = bookseries::table
        .find(*id)
        .inner_join(series::table)
        .select((series::name, bookseries::number, series::id))
        .first(&mut conn)
        .await
        .optional()?;

    let image_url = super::components::make_image_url(&state, *id, &user);

    let summary = ammonia::clean(&book.summary);

    let authors = BookAuthor::belonging_to(&book)
        .inner_join(author::table)
        .select(Author::as_select())
        .load::<Author>(&mut conn)
        .await?;

    let tags = BookTag::belonging_to(&book)
        .inner_join(tag::table)
        .select(tag::name)
        .load::<String>(&mut conn)
        .await?;

    Ok(app_page(
        super::Page::Books,
        &user,
        html! {
            .container.text-center {
                h2 {
                    (book.title)
                    a .ms-2.btn.btn-primary href=(format!("{}/edit", *id)) { i .bi.bi-pencil {} }
                }
                ."mb-2" {
                    img style="height: 24rem" src=(image_url) alt="cover art";
                }
                .container {
                    @if let Some((name, idx, id)) = series {
                        span .fs-3 {
                            a .link-light.link-offset-1
                                href=(format!("/series/{id}")) {
                                (name)
                            }
                            (format!(" #{idx}"))
                        }
                        br;
                    }
                    @for (i, author) in authors.iter().enumerate() {
                        @if i != 0 {
                            ", "
                        }
                        span .fs-4 {
                            a .link-light.link-offset-1
                                href=(format!("/author/{}", author.id)) {
                                (author.name)
                            }
                        }
                    }
                    br;
                    @if book.owned || book.read {
                        @if book.owned {
                            .span .badge.text-bg-info.me-2 { "Owned" }
                        }
                        @if book.read {
                            .span .badge.text-bg-info.me-2 { "Read" }
                        }
                        br;
                    }
                    @for tag in tags {
                        span .badge.text-bg-primary.me-2 { (tag) }
                    }
                }
                .container."mb-2" {
                    (PreEscaped(summary))
                    hr;
                    .text-start {
                        @if let Some(date) = book.published {
                            "Publication date: " (date.format("%d/%m/%Y"))
                            br;
                        }
                        @if let Some(publisher) = book.publisher {
                            "Publisher: " (publisher)
                            br;
                        }
                        @if let Some(language) = book.language {
                            "Language: " (language)
                            br;
                        }
                        @if let Some(page_count) = book.pagecount {
                            "Page count: " (page_count)
                            br;
                        }
                        "ISBN: " (book.isbn)
                    }
                }
            }
        },
    ))
}
