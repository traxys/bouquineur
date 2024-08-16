use std::collections::HashMap;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use maud::{html, PreEscaped};
use uuid::Uuid;

use crate::{
    metadata::NullableBookDetails,
    models::{Author, BookAuthor, BookPreview, BookSeries, SeriesInfo, User},
    schema::{author, book, bookauthor, booktag, series, tag},
    State,
};

use super::{RouteError, NO_COVER};

async fn author_list(state: &State, user: &User) -> Result<Vec<String>, RouteError> {
    let mut conn = state.db.get().await?;

    // List of books of an user
    let user_books = book::table.filter(book::owner.eq(user.id)).select(book::id);

    // List of authors for an user
    let book_author_ids = bookauthor::table
        .filter(bookauthor::book.eq_any(user_books))
        .select(bookauthor::author);

    let authors: Vec<String> = author::table
        .filter(author::id.eq_any(book_author_ids))
        .select(author::name)
        .load(&mut conn)
        .await?;

    Ok(authors)
}

async fn tag_list(state: &State, user: &User) -> Result<Vec<String>, RouteError> {
    let mut conn = state.db.get().await?;

    // List of books of an user
    let user_books = book::table.filter(book::owner.eq(user.id)).select(book::id);

    // List of tags for an user
    let book_tag_ids = booktag::table
        .filter(booktag::book.eq_any(user_books))
        .select(booktag::tag);

    let authors: Vec<String> = tag::table
        .filter(tag::id.eq_any(book_tag_ids))
        .select(tag::name)
        .load(&mut conn)
        .await?;

    Ok(authors)
}

async fn series_list(state: &State, user: &User) -> Result<Vec<String>, RouteError> {
    let mut conn = state.db.get().await?;

    Ok(series::table
        .filter(series::owner.eq(user.id))
        .select(series::name)
        .load(&mut conn)
        .await?)
}

fn list_input(
    id: &str,
    placeholder: &str,
    defaults: &[String],
    completions: &[String],
    remove_label: &str,
) -> maud::Markup {
    let list_id = format!("{id}CompleteList");
    let values_id = format!("{id}Values");
    let input_id = format!("{id}Input");

    html! {
        input #(input_id) .form-control.awesomplete."mb-2" list=(list_id) data-tabSelect="true"
            placeholder=(placeholder);
        datalist #(list_id) {
            @for possible in completions {
                option { (possible) }
            }
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

                function {id}Add(value) {{
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

                {id}Completing = false

                {id}Input.addEventListener("awesomplete-highlight", function(event) {{
                    {id}Completing = true
                }})

                {id}Input.addEventListener("awesomplete-close", function(event) {{
                    {id}Completing = false
                }})
                
                {id}Input.addEventListener("keydown", function(event) {{
                    if (event.key == "Enter") {{
                        event.preventDefault();

                        const value = {id}Input.value

                        if (value == '' || {id}Completing)
                            return

                        {id}Add(value)
                    }}
                }})

                {id}Input.addEventListener("change", function(event) {{
                    event.preventDefault();

                    const value = {id}Input.value

                    if (value == '')
                        return

                    {id}Add(value)
                }})
            "#)))
        }
    }
}

pub async fn book_form(
    state: &State,
    user: &User,
    details: NullableBookDetails,
    submit: &str,
) -> Result<maud::Markup, RouteError> {
    let image = details
        .covert_art_b64
        .as_ref()
        .unwrap_or_else(|| &*NO_COVER);

    let authors = author_list(state, user).await?;
    let tags = tag_list(state, user).await?;
    let series = series_list(state, user).await?;

    let (series_name, series_number) = details.series.unzip();

    Ok(
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
            .form-check {
                input .form-check-input type="checkbox" name="read_box" #readBox checked[details.read];
                label .form-check-label for="readBox" { "Read" }
            }
            .form-check {
                input .form-check-input type="checkbox" name="owned_box" #ownedBox checked[details.owned];
                label .form-check-label for="ownedBox" { "Owned" }
            }
            .row."g-2"."mb-2" {
                .col {
                    input #seriesInput .form-control.awesomplete."me-1" list="seriesList" name="series_name"
                        placeholder="Series" value=[series_name];
                    datalist #seriesList {
                        @for series in series {
                            option { (series) }
                        }
                    }
                }
                .col {
                    input #seriesVolume name="series_volume" .form-control placeholder="Series volume"
                        type="number" value=[series_number];
                }
                script {
                    (PreEscaped(r#"
                        const seriesName = document.getElementById('seriesInput')
                        const seriesVolume = document.getElementById('seriesVolume')
                        const requiredOnLoad = seriesName.value != "" || seriesVolume.value != ""

                        seriesName.required = requiredOnLoad
                        seriesVolume.required = requiredOnLoad

                        function setSeriesRequired() {
                            const required = seriesName.value != "" || seriesVolume.value != ""
                            seriesName.required = required
                            seriesVolume.required = required
                        }

                        seriesName.addEventListener('input', setSeriesRequired)
                        seriesVolume.addEventListener('input', setSeriesRequired)
                    "#))
                }
            }
            (list_input("author", "Author name", &details.authors, &authors, "Remove author"))
            (list_input("tag", "Tag", &details.tags, &tags, "Remove tag"))
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
            input type="submit" .btn.btn-primary value=(submit);
        } },
    )
}

pub const NO_SORT: Option<fn(&BookPreview, &BookPreview) -> std::cmp::Ordering> = None;

pub async fn book_cards_for<F>(
    state: &State,
    user: &User,
    books: &[BookPreview],
    sort_by: Option<F>,
) -> Result<maud::Markup, RouteError>
where
    F: Fn(&BookPreview, &BookPreview) -> std::cmp::Ordering,
{
    let mut conn = state.db.get().await?;

    let authors = BookAuthor::belonging_to(books)
        .inner_join(author::table)
        .select((BookAuthor::as_select(), Author::as_select()))
        .load::<(BookAuthor, Author)>(&mut conn)
        .await?;

    let series = BookSeries::belonging_to(books)
        .inner_join(series::table)
        .select((BookSeries::as_select(), SeriesInfo::as_select()))
        .load::<(BookSeries, SeriesInfo)>(&mut conn)
        .await?;

    #[derive(Debug)]
    struct BookSeriesInfo {
        name: String,
        volume: i32,
        series: Uuid,
    }

    let book_series = series
        .into_iter()
        .map(|(bookseries, series)| {
            (
                bookseries.book,
                BookSeriesInfo {
                    name: series.name,
                    volume: bookseries.number,
                    series: bookseries.series,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let mut book_data: Vec<_> = authors
        .grouped_by(books)
        .into_iter()
        .zip(books)
        .map(|(a, book)| {
            let image_path = state
                .config
                .metadata
                .image_dir
                .join(user.id.to_string())
                .join(format!("{}.jpg", book.id));

            let image_url = match image_path.exists() {
                true => format!("/images/{}", book.id),
                false => "/images/not_found".to_string(),
            };

            Ok((
                book,
                image_url,
                a.into_iter().map(|(_, author)| author).collect::<Vec<_>>(),
                book_series.get(&book.id),
            ))
        })
        .collect::<Result<_, RouteError>>()?;

    if let Some(f) = sort_by {
        book_data.sort_unstable_by(|(book_a, _, _, _), (book_b, _, _, _)| f(book_a, book_b));
    }

    Ok(html! {
        .container {
            .row.row-cols-auto.justify-content-center.justify-content-md-start {
                @for (book, image, authors, series) in book_data {
                    ."col"."mb-2" {
                        .card."h-100" style="width: 9.6rem;" {
                            img src=(image) .card-img-top alt="book cover"
                                style="height: 14.4rem; width: 9.6rem;";
                            .card-body {
                                h6 .card-title {
                                    a .nav-link.fs-5 href=(format!("/book/{}", book.id)) {
                                        (book.title)
                                    }
                                }
                                p .card-text {
                                    @for author in authors {
                                        a href=(format!("/author/{}", author.id))
                                          .nav-link {
                                            (author.name)
                                        }
                                    }
                                }
                            }
                            @if series.is_some() || book.read || book.owned {
                                .card-footer.d-flex.justify-content-evenly {
                                    @if let Some(series) = series {
                                        a href=(format!("/series/{}", series.series))
                                          .link-light
                                          data-bs-toggle="tooltip"
                                          data-bs-title=(format!("{} #{}", series.name, series.volume))
                                        {
                                            i .bi.bi-collection {}
                                        }
                                    }
                                    @if book.owned {
                                        i .bi.bi-check-circle
                                            data-bs-toggle="tooltip"
                                            data-bs-title="Owned" {}
                                    }
                                    @if book.read {
                                        i .bi.bi-book-fill
                                            data-bs-toggle="tooltip"
                                            data-bs-title="Read" {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}
