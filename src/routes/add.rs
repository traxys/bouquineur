use std::cmp::Ordering;

use axum::extract::Query;
use diesel::prelude::*;
use diesel_async::{scoped_futures::ScopedFutureExt, AsyncConnection, RunQueryDsl};
use maud::html;
use uuid::Uuid;

use crate::{
    metadata::{fetch_metadata, MetadataProvider, NullableBookDetails},
    models::{BookAuthor, BookTag, User},
    routes::components::book_form,
    schema::{author, book, bookauthor, booktag, tag},
};

use super::{app_page, icons, BookInfo, Page, RouteError, State};

pub(crate) async fn do_add_book(
    state: State,
    user: User,
    data: BookInfo,
) -> Result<axum::response::Redirect, RouteError> {
    let mut conn = state.db.get().await?;

    conn.transaction(|c| {
        async {
            diesel::insert_into(author::table)
                .values(&data.authors)
                .on_conflict_do_nothing()
                .execute(c)
                .await?;

            diesel::insert_into(tag::table)
                .values(&data.tags)
                .on_conflict_do_nothing()
                .execute(c)
                .await?;

            let book_id: Uuid = diesel::insert_into(book::table)
                .values(data.book)
                .returning(book::id)
                .get_result(c)
                .await?;

            let author_ids: Vec<i32> = author::table
                .filter(author::name.eq_any(&data.authors))
                .select(author::id)
                .load(c)
                .await?;

            diesel::insert_into(bookauthor::table)
                .values(
                    &author_ids
                        .into_iter()
                        .map(|author| BookAuthor {
                            book: book_id,
                            author,
                        })
                        .collect::<Vec<_>>(),
                )
                .execute(c)
                .await?;

            let tag_ids: Vec<i32> = tag::table
                .filter(tag::name.eq_any(&data.tags))
                .select(tag::id)
                .load(c)
                .await?;

            diesel::insert_into(booktag::table)
                .values(
                    &tag_ids
                        .into_iter()
                        .map(|tag| BookTag { book: book_id, tag })
                        .collect::<Vec<_>>(),
                )
                .execute(c)
                .await?;

            let image_dir = state.config.metadata.image_dir.join(user.id.to_string());

            std::fs::create_dir_all(&image_dir)
                .map_err(|e| RouteError::ImageSave(image::ImageError::IoError(e)))?;

            let mut image_path = image_dir.join(book_id.to_string());
            image_path.set_extension("jpg");

            if let Some(img) = data.image {
                tokio::task::block_in_place(|| -> Result<_, RouteError> {
                    img.save(image_path).map_err(RouteError::ImageSave)?;
                    Ok(())
                })?;
            }

            Ok::<_, RouteError>(())
        }
        .scope_boxed()
    })
    .await?;

    Ok(axum::response::Redirect::to("/"))
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct IsbnRequest {
    isbn: Option<String>,
    provider: Option<MetadataProvider>,
}

pub(crate) async fn add_book(
    state: State,
    user: User,
    query: Query<IsbnRequest>,
) -> Result<maud::Markup, RouteError> {
    let has_provider = match &state.config.metadata.providers {
        None => true,
        Some(list) => !list.is_empty(),
    };

    let providers = state
        .config
        .metadata
        .providers
        .as_deref()
        .unwrap_or(MetadataProvider::all());

    let default_provider = match providers.len().cmp(&1) {
        Ordering::Equal => providers[0],
        _ => state
            .config
            .metadata
            .default_provider
            .unwrap_or(MetadataProvider::Calibre),
    };

    enum SearchResult {
        Found,
        NotFound,
        AlreadyExists,
    }

    let (res, book_details) = match &query.isbn {
        Some(isbn) if has_provider => {
            let isbn = isbn.replace('-', "");

            let mut conn = state.db.get().await?;

            let found: i64 = book::table
                .filter(book::owner.eq(user.id).and(book::isbn.eq(&isbn)))
                .count()
                .get_result(&mut conn)
                .await?;

            if found == 0 {
                fetch_metadata(
                    &state.config,
                    &isbn,
                    query.provider.unwrap_or(default_provider),
                )
                .await?
                .map(|v| (SearchResult::Found, v))
                .unwrap_or_else(|| (SearchResult::NotFound, Default::default()))
            } else {
                (SearchResult::AlreadyExists, Default::default())
            }
        }
        _ => (SearchResult::Found, (NullableBookDetails::default())),
    };

    Ok(app_page(
        Page::AddBook,
        &user,
        html! {
            #isbnModal .modal.fade tabindex="-1" aria-labelledby="isbnModalLabel" aria-hidden="true" {
                .modal-dialog.modal-dialog-centered { .modal-content {
                    .modal-header {
                        h1 .modal-title."fs-5" #isbnModalLabel {"Load a book from an ISBN"}
                        button type="button" .btn-close data-bs-dismiss="modal" aria-label="Cancel" {}
                    }
                    .modal-body {
                        form #isbnModalForm {
                            .form-floating {
                                input name="isbn"
                                        type="text"
                                        .form-control
                                        #isbnSearch
                                        placeholder="978-3-16-148410-0";
                                label for="isbnSearch" { "ISBN" }
                            }
                        }
                    }
                    .modal-footer {
                        button type="button" .btn.btn-secondary data-bs-dismiss="modal" { "Cancel" }
                        button type="submit" form="isbnModalForm" .btn.btn-primary { "Load" }
                    }
                }  }
            }

            #scanModal .modal.fade tabindex="-1" aria-labelledby="scanModalLabel" aria-hidden="true" {
                .modal-dialog.modal-dialog-centered { .modal-content {
                    .modal-header {
                        h1 .modal-title."fs-5" #scanModalLabel {"Load a book from an ISBN barcode"}
                        button type="button" .btn-close data-bs-dismiss="modal" aria-label="Cancel" {}
                    }
                    .modal-body {
                        video #scanVideo width="300" height="200" style="border: 1px solid gray" {}
                    }
                    .modal-footer {
                        button type="button" .btn.btn-secondary data-bs-dismiss="modal" { "Cancel" }
                    }
                }  }
            }

            @match res {
                SearchResult::Found => {},
                SearchResult::NotFound => {
                    .alert.alert-warning role="alert" {
                        "The requested ISBN was not found"
                    }
                },
                SearchResult::AlreadyExists => {
                    .alert.alert-warning role="alert" {
                        "The requested ISBN is already in the database"
                    }
                },
            }

            .d-flex.flex-column {
                @if has_provider {
                    @if providers.len() > 1 {
                        .container {
                            ul .list-group."mb-2" {
                                li .list-group-item {
                                    "Metadata provider"
                                }
                                @for &provider in providers {
                                    li .list-group-item {
                                        @let id = format!("{provider}Radio");
                                        input .form-check-input."me-1" type="radio" #(id)
                                              name="provider" value=(provider.serialized())
                                              form="isbnModalForm" checked[provider == default_provider];
                                        label .form-check-label for=(id) {
                                            (provider.to_string())
                                        }
                                    }
                                }
                            }
                        }
                    }
                    .d-flex.justify-content-center {
                        button .btn.btn-primary.me-2 data-bs-toggle="modal" data-bs-target="#isbnModal" {
                            (icons::bi_123()) "Load from ISBN"
                        }
                        button .btn.btn-primary data-bs-toggle="modal" data-bs-target="#scanModal" {
                            (icons::bi_upc_scan()) "Scan ISBN"
                        }
                    }
                }
                (book_form(&state, &user, book_details, "Add Book").await?)
            }

            script {
                (maud::PreEscaped(include_str!("./barcode.js")))
            }
        },
    ))
}
