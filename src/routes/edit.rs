use std::{fs::OpenOptions, io::BufWriter};

use axum::{extract::Path, response::Redirect};
use base64::prelude::*;
use diesel::prelude::*;
use diesel_async::{scoped_futures::ScopedFutureExt, AsyncConnection, RunQueryDsl};
use maud::html;
use uuid::Uuid;

use crate::{
    metadata::NullableBookDetails,
    models::{BookAuthor, BookComplete, BookId, BookSeries, BookTag, Series, User},
    routes::components::book_form,
    schema::{author, book, bookauthor, bookseries, booktag, series, tag},
    State,
};

use super::{app_page, BookInfo, RouteError};

pub(crate) async fn do_edit_book(
    state: State,
    user: User,
    id: Path<Uuid>,
    data: BookInfo,
) -> Result<Redirect, RouteError> {
    let mut conn = state.db.get().await?;

    let has_book: i64 = book::table
        .filter(book::owner.eq(user.id))
        .find(*id)
        .count()
        .get_result(&mut conn)
        .await?;

    if has_book == 0 {
        return Err(RouteError::NotFound);
    }

    conn.transaction(|c| {
        async {
            diesel::delete(bookauthor::table)
                .filter(bookauthor::book.eq(*id))
                .execute(c)
                .await?;

            diesel::delete(booktag::table)
                .filter(booktag::book.eq(*id))
                .execute(c)
                .await?;

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

            diesel::update(&BookId { id: *id })
                .set(data.book)
                .execute(c)
                .await?;

            if let Some((name, volume)) = data.series {
                let series = Series {
                    name,
                    owner: user.id,
                };

                let series_id = diesel::insert_into(series::table)
                    .values(&series)
                    .on_conflict((series::owner, series::name))
                    .do_update()
                    .set(&series)
                    .returning(series::id)
                    .get_result(c)
                    .await?;

                let book_series = BookSeries {
                    book: *id,
                    series: series_id,
                    number: volume,
                };

                diesel::insert_into(bookseries::table)
                    .values(&book_series)
                    .on_conflict(bookseries::book)
                    .do_update()
                    .set(&book_series)
                    .execute(c)
                    .await?;
            }

            let author_ids: Vec<i32> = author::table
                .filter(author::name.eq_any(&data.authors))
                .select(author::id)
                .load(c)
                .await?;

            diesel::insert_into(bookauthor::table)
                .values(
                    &author_ids
                        .into_iter()
                        .map(|author| BookAuthor { book: *id, author })
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
                        .map(|tag| BookTag { book: *id, tag })
                        .collect::<Vec<_>>(),
                )
                .execute(c)
                .await?;

            let image_dir = state.config.metadata.image_dir.join(user.id.to_string());

            std::fs::create_dir_all(&image_dir)
                .map_err(|e| RouteError::ImageSave(image::ImageError::IoError(e)))?;

            let mut image_path = image_dir.join(id.to_string());
            image_path.set_extension("jpg");

            if let Some(img) = data.image {
                tokio::task::block_in_place(|| -> Result<_, RouteError> {
                    let file = OpenOptions::new()
                        .truncate(true)
                        .write(true)
                        .read(true)
                        .open(&image_path)
                        .map_err(|e| RouteError::ImageSave(image::ImageError::IoError(e)))?;

                    img.write_to(&mut BufWriter::new(file), image::ImageFormat::Jpeg)
                        .map_err(RouteError::ImageSave)?;

                    Ok(())
                })?;
            }

            Ok::<_, RouteError>(())
        }
        .scope_boxed()
    })
    .await?;

    Ok(Redirect::to(&format!("/book/{}", *id)))
}

pub(crate) async fn edit_book(
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

    let series = bookseries::table
        .find(*id)
        .inner_join(series::table)
        .select((series::name, bookseries::number))
        .get_result(&mut conn)
        .await
        .optional()?;

    let authors = BookAuthor::belonging_to(&book)
        .inner_join(author::table)
        .select(author::name)
        .load::<String>(&mut conn)
        .await?;

    let tags = BookTag::belonging_to(&book)
        .inner_join(tag::table)
        .select(tag::name)
        .load::<String>(&mut conn)
        .await?;

    let image_path = state
        .config
        .metadata
        .image_dir
        .join(user.id.to_string())
        .join(format!("{}.jpg", *id));

    let covert_art_b64 = match image_path.exists() {
        true => Some(BASE64_STANDARD.encode(tokio::fs::read(image_path).await?)),
        false => None,
    };

    let book_details = NullableBookDetails {
        isbn: Some(book.isbn),
        title: Some(book.title),
        authors,
        tags,
        summary: Some(book.summary),
        published: book.published,
        publisher: book.publisher,
        language: book.language,
        google_id: book.googleid,
        amazon_id: book.amazonid,
        librarything_id: book.librarythingid,
        page_count: book.pagecount,
        owned: book.owned,
        read: book.read,
        covert_art_b64,
        series,
    };

    Ok(app_page(
        super::Page::Books,
        &user,
        html! {
            (book_form(&state, &user, book_details, "Edit book").await?)
        },
    ))
}
