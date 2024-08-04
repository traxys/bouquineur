use std::{io::Cursor, sync::LazyLock};

use axum::{
    body::Bytes,
    extract::{Multipart, Query},
};
use base64::prelude::*;
use chrono::NaiveDate;
use diesel::prelude::*;
use diesel_async::{scoped_futures::ScopedFutureExt, AsyncConnection, RunQueryDsl};
use maud::html;
use uuid::Uuid;

use crate::{
    metadata::{fetch_metadata, NullableBookDetails},
    models::{AuthorName, Book, BookAuthor, BookTag, TagName, User},
    schema::{author, book, bookauthor, booktag, tag},
};

use super::{app_page, icons, Page, RouteError, State};

static NO_COVER: LazyLock<String> = LazyLock::new(|| {
    let image = include_bytes!("../no_cover.jpg");
    BASE64_STANDARD.encode(image)
});

fn list_input(
    id: &str,
    placeholder: &str,
    defaults: &[String],
    remove_label: &str,
) -> maud::Markup {
    let list_id = format!("{id}CompleteList");
    let values_id = format!("{id}Values");
    let input_id = format!("{id}Input");

    html! {
        input #(input_id) .form-control.awesomplete."mb-2" list=(list_id)
            placeholder=(placeholder);
        datalist #(list_id) {
        }
        ul #(values_id) .list-group."mb-3" {
            @for item in defaults {
                li .list-group-item.d-flex.justify-content-between.align-items-center {
                    (item)
                    span {
                        button type="button" .btn-close aria-label=(remove_label) onclick=(format!("delete{id}(event)"));
                    }
                    input type="hidden" name=(id) value=(item);
                }
            }
        }
        script {
            (maud::PreEscaped(format!(r#"
                {id}Input = document.getElementById("{input_id}")
                {id}List = document.getElementById("{values_id}")

                function delete{id}(event) {{
                    event.srcElement.parentNode.parentNode.remove()
                }}
                
                {id}Input.addEventListener("keydown", function(event) {{
                    if (event.key == "Enter") {{
                        event.preventDefault();

                        const value = {id}Input.value

                        if (value == '')
                            return

                        const listItem = document.createElement("li")
                        listItem.className = "list-group-item d-flex justify-content-between align-items-center"

                        const valueEl = document.createTextNode(value);
                        {id}Input.value = ''
                        listItem.appendChild(valueEl)

                        const removeSpan = document.createElement("span")
                        const removeButton = document.createElement("button")
                        removeButton.type = "button"
                        removeButton.className = "btn-close"
                        removeButton.ariaLabel = "{remove_label}"
                        removeButton.addEventListener("click", delete{id})
                        removeSpan.appendChild(removeButton)
                        listItem.appendChild(removeSpan)

                        const listInput = document.createElement("input")
                        listInput.type = "hidden"
                        listInput.name = "{id}"
                        listInput.value = value
                        listItem.appendChild(listInput)

                        {id}List.appendChild(listItem)
                    }}
                }})
            "#)))
        }
    }
}

async fn book_form(state: &State, user: &User, details: NullableBookDetails) -> maud::Markup {
    let image = details
        .covert_art_b64
        .as_ref()
        .unwrap_or_else(|| &*NO_COVER);

    html! { form .container-sm.align-items-center method="POST" enctype="multipart/form-data" .mt-2 {
        .text-center.d-flex.flex-column."mb-2" {
            label for="coverArtInput" .form-label {"Cover art"}
            div {
                img .img-fluid."mb-2"
                    #coverArt
                    style="height:400px;"
                    alt="Cover Art"
                    src=(format!("data:image/jpg;base64,{image}"));
            }
            input .form-control accept="image/*" type="file" name="user_cover" #coverArtInput;
            script {
                (maud::PreEscaped(r#"
                    coverArt = document.getElementById("coverArt")
                    coverArtInput = document.getElementById("coverArtInput")
            
                    coverArtInput.onchange = evt => {
                        const [file] = coverArtInput.files
                        if (file) {
                            coverArt.src = URL.createObjectURL(file)
                        }
                    }
                "#))
            }
            @if let Some(b64) = details.covert_art_b64 {
                input type="hidden" value=(b64) name="fetched_cover";
            }
        }
        .form-floating."mb-2" {
            input .form-control required #title name="title" type="text"
                    placeholder="Title" value=[details.title];
            label for="title" { "Title" }
        }
        .form-floating."mb-2" {
            input .form-control required #isbn name="isbn" type="text"
                    placeholder="ISBN" value=[details.isbn];
            label for="isbn" { "ISBN" }
        }
        .form-floating."mb-2" {
            textarea .form-control placeholder="Book summary" #summary style="height: 150px" name="summary" {
                (details.summary.unwrap_or_default())
            }
            label for="summary" { "Summary" }
        }
        (list_input("author", "Author name", &details.authors, "Remove author"))
        (list_input("tag", "Tag", &details.tags, "Remove tag"))
        .form-floating."mb-2" {
            input #published name="published" type="date" .form-control placeholder="1970-01-01"
                  value=[details.published.map(|d| d.format("%Y-%m-%d"))];
            label for="published" {"Publication Date"}
        }
        .form-floating."mb-2" {
            input .form-control #publisher name="publisher" type="text"
                    placeholder="Publisher" value=[details.publisher];
            label for="publisher" { "Publisher" }
        }
        .form-floating."mb-2" {
            input .form-control #language name="language" type="text"
                    placeholder="Language" value=[details.language];
            label for="language" { "Language" }
        }
        .form-floating."mb-2" {
            input .form-control #googleID name="google_id" type="text"
                    placeholder="Google ID" value=[details.google_id];
            label for="googleID" { "Google ID" }
        }
        .form-floating."mb-2" {
            input .form-control #amazonID name="amazon_id" type="text"
                    placeholder="Amazon ID" value=[details.amazon_id];
            label for="amazonID" { "Amazon ID" }
        }
        .form-floating."mb-2" {
            input .form-control #librarythingId name="librarything_id" type="text"
                    placeholder="Librarything ID" value=[details.librarything_id];
            label for="librarythingId" { "Librarything ID" }
        }
        .form-floating."mb-2" {
            input .form-control #pageCount name="page_count" type="number"
                    placeholder="Page Count" value=[details.page_count];
            label for="pageCount" { "Page Count" }
        }
        input type="submit" .btn.btn-primary value="Add Book";
    } }
}

pub(crate) async fn do_add_book(
    state: State,
    user: User,
    mut multipart: Multipart,
) -> Result<axum::response::Redirect, RouteError> {
    enum CoverArt {
        User(Bytes),
        Fetched(String),
    }

    #[derive(Default)]
    struct BookData {
        cover_art: Option<CoverArt>,
        title: Option<String>,
        isbn: Option<String>,
        summary: String,
        authors: Vec<AuthorName>,
        tags: Vec<TagName>,
        publication_date: Option<NaiveDate>,
        publisher: Option<String>,
        language: Option<String>,
        google_id: Option<String>,
        amazon_id: Option<String>,
        librarything_id: Option<String>,
        page_count: Option<i32>,
    }

    let mut data = BookData::default();

    let load = |s: String| if s.is_empty() { None } else { Some(s) };

    while let Some(field) = multipart.next_field().await? {
        let Some(name) = field.name() else {
            tracing::warn!("Unamed multipart field");
            continue;
        };

        match name {
            "user_cover" => {
                let cover = field.bytes().await?;
                if !cover.is_empty() {
                    data.cover_art = Some(CoverArt::User(cover));
                }
            }
            "fetched_cover" => {
                if data.cover_art.is_none() {
                    data.cover_art = Some(CoverArt::Fetched(field.text().await?));
                }
            }
            "title" => data.title = load(field.text().await?),
            "isbn" => data.isbn = load(field.text().await?),
            "summary" => data.summary = field.text().await?,
            "author" => data.authors.push(AuthorName {
                name: field.text().await?,
            }),
            "tag" => data.tags.push(TagName {
                name: field.text().await?,
            }),
            "published" => {
                let text = field.text().await?;
                if !text.is_empty() {
                    data.publication_date = Some(NaiveDate::parse_from_str(&text, "%Y-%m-%d")?)
                }
            }
            "publisher" => data.publisher = load(field.text().await?),
            "language" => data.language = load(field.text().await?),
            "google_id" => data.google_id = load(field.text().await?),
            "amazon_id" => data.amazon_id = load(field.text().await?),
            "librarything_id" => data.librarything_id = load(field.text().await?),
            "page_count" => {
                let text = field.text().await?;
                if !text.is_empty() {
                    data.page_count = Some(text.parse()?)
                }
            }
            _ => {
                tracing::warn!("Unknown field {:?}", field.name());
            }
        }
    }

    let book = Book {
        owner: user.id,
        isbn: data.isbn.ok_or(RouteError::MissingField)?,
        title: data.title.ok_or(RouteError::MissingField)?,
        summary: data.summary,
        published: data.publication_date,
        publisher: data.publisher,
        language: data.language,
        googleid: data.google_id,
        amazonid: data.amazon_id,
        librarythingid: data.librarything_id,
        pagecount: data.page_count,
    };

    let mut conn = state.db.get().await?;

    let image = match data.cover_art {
        Some(CoverArt::User(bytes)) => Some(
            image::ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()
                .map_err(RouteError::ImageDetection)?
                .decode()?,
        ),
        Some(CoverArt::Fetched(data)) => {
            let data = BASE64_STANDARD.decode(data)?;

            Some(
                image::ImageReader::new(Cursor::new(data))
                    .with_guessed_format()
                    .map_err(RouteError::ImageDetection)?
                    .decode()?,
            )
        }
        None => None,
    };

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
                .values(book)
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

            if let Some(img) = image {
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
}

pub(crate) async fn add_book(
    state: State,
    user: User,
    isbn: Query<IsbnRequest>,
) -> Result<maud::Markup, RouteError> {
    let (not_found, book_details) = match &isbn.isbn {
        None => (false, (NullableBookDetails::default())),
        Some(isbn) => fetch_metadata(&state.config, isbn)
            .await?
            .map(|v| (false, v))
            .unwrap_or_else(|| (true, Default::default())),
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

            @if not_found {
                .alert.alert-warning role="alert" {
                    "The requested ISBN was not found"
                }
            }

            .d-flex.flex-column {
                .d-flex.justify-content-center {
                    button .btn.btn-primary.me-2 data-bs-toggle="modal" data-bs-target="#isbnModal" {
                        (icons::bi_123()) "Load from ISBN"
                    }
                    button .btn.btn-primary data-bs-toggle="modal" data-bs-target="#scanModal" {
                        (icons::bi_upc_scan()) "Scan ISBN"
                    }
                }
                (book_form(&state, &user, book_details).await)
            }

            script {
                (maud::PreEscaped(include_str!("./barcode.js")))
            }
        },
    ))
}
