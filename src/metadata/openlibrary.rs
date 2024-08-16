use base64::prelude::*;
use chrono::NaiveDate;
use reqwest::StatusCode;

use crate::OpenLibraryConfig;

use super::NullableBookDetails;

#[derive(thiserror::Error, Debug)]
pub enum OpenLibraryMetadataError {
    #[error("Could not make open library client")]
    MakeClient(#[source] reqwest::Error),
    #[error("Could not parse JSON response ({0})")]
    Json(#[from] serde_path_to_error::Error<serde_json::Error>),
    #[error("Error in HTTP request")]
    RequestError(#[from] reqwest::Error),
    #[error("Work is missing from edition")]
    MissingWork,
    #[error("Expected resource was not found")]
    NotFound,
}

#[derive(serde::Deserialize, Debug)]
struct Text {
    value: String,
}

#[derive(serde::Deserialize, Debug)]
#[serde(untagged)]
enum Description {
    Str(String),
    Value(Text),
}

impl Description {
    fn text(self) -> String {
        match self {
            Description::Str(s) => s,
            Description::Value(v) => v.value,
        }
    }
}

#[derive(serde::Deserialize, Debug)]
struct Reference {
    key: String,
}

#[derive(serde::Deserialize, Debug)]
struct AuthorReference {
    author: Reference,
    #[serde(rename = "type")]
    ty: Reference,
}

#[derive(serde::Deserialize, Debug)]
struct Work {
    #[serde(default)]
    description: Option<Description>,
    #[serde(default)]
    subjects: Vec<String>,
    #[serde(default)]
    authors: Vec<AuthorReference>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct Edition {
    #[serde(default)]
    publish_date: Option<String>,
    #[serde(default)]
    publishers: Vec<String>,
    #[serde(default)]
    languages: Vec<Reference>,
    #[serde(default)]
    number_of_pages: Option<i32>,
    #[serde(default)]
    covers: Vec<i64>,
    #[serde(default)]
    works: Vec<Reference>,
}

#[derive(serde::Deserialize, Debug)]
struct Author {
    #[serde(default)]
    name: Option<String>,
}

async fn fetch(
    url: &str,
    client: &reqwest::Client,
) -> Result<Option<String>, OpenLibraryMetadataError> {
    let rsp = client.get(url).send().await?;

    if rsp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    Ok(Some(rsp.error_for_status()?.text().await?))
}

const OPEN_LIBRARY: &str = "https://openlibrary.org";

pub(super) async fn fetch_metadata(
    config: &OpenLibraryConfig,
    isbn: &str,
) -> Result<Option<NullableBookDetails>, OpenLibraryMetadataError> {
    tracing::debug!("Querying OpenLibrary for isbn '{isbn}'");

    let user_agent = format!("github.com/traxys/bouquineur ({})", config.contact);
    let client = reqwest::Client::builder()
        .user_agent(user_agent)
        .build()
        .map_err(OpenLibraryMetadataError::MakeClient)?;

    let Some(edition) = fetch(&format!("{OPEN_LIBRARY}/isbn/{isbn}.json"), &client).await? else {
        return Ok(None);
    };

    tracing::trace!("Edition:\n{edition}");
    let de = &mut serde_json::Deserializer::from_str(&edition);
    let edition: Edition = match serde_path_to_error::deserialize(de) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Could not parse edition: {e:?}");
            return Err(e.into());
        }
    };
    tracing::debug!("Parsed edition: {edition:?}");

    if edition.works.is_empty() {
        return Err(OpenLibraryMetadataError::MissingWork);
    }

    if edition.works.len() > 1 {
        tracing::warn!("More than one work in edition: {:?}", edition.works)
    }

    let work = fetch(
        &format!("{OPEN_LIBRARY}/{}.json", edition.works[0].key),
        &client,
    )
    .await?
    .ok_or(OpenLibraryMetadataError::NotFound)?;

    tracing::trace!("Work:\n{work}");
    let de = &mut serde_json::Deserializer::from_str(&work);
    let work: Work = match serde_path_to_error::deserialize(de) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Could not parse work: {e:?}");
            return Err(e.into());
        }
    };
    tracing::debug!("Parsed work: {work:?}");

    let mut authors = Vec::new();
    for author in &work.authors {
        if author.ty.key == "/type/author_role" {
            let author = fetch(
                &format!("{OPEN_LIBRARY}/{}.json", author.author.key),
                &client,
            )
            .await?
            .ok_or(OpenLibraryMetadataError::NotFound)?;

            tracing::trace!("Author:\n{author}");
            let de = &mut serde_json::Deserializer::from_str(&author);
            let author: Author = match serde_path_to_error::deserialize(de) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Could not parse author: {e:?}");
                    return Err(e.into());
                }
            };
            tracing::debug!("Parsed author: {author:?}");

            if let Some(author) = author.name {
                authors.push(author);
            }
        }
    }

    let published = match edition.publish_date {
        None => None,
        Some(v) => match parse_datetime::parse_datetime(&v) {
            Err(_) => match human_date_parser::from_human_time(&v) {
                Err(_) => match v.parse::<i32>() {
                    Err(_) => None,
                    Ok(v) => NaiveDate::from_ymd_opt(v, 1, 1),
                },
                Ok(v) => match v {
                    human_date_parser::ParseResult::DateTime(dt) => Some(dt.date_naive()),
                    human_date_parser::ParseResult::Date(d) => Some(d),
                    human_date_parser::ParseResult::Time(_) => None,
                },
            },
            Ok(v) => Some(v.date_naive()),
        },
    };

    let covert_art_b64 = match edition.covers.is_empty() {
        true => None,
        false => {
            let cover = client
                .get(&format!(
                    "https://covers.openlibrary.org/b/id/{}-M.jpg",
                    edition.covers[0]
                ))
                .send()
                .await?
                .bytes()
                .await?;

            Some(BASE64_STANDARD.encode(&cover))
        }
    };

    Ok(Some(NullableBookDetails {
        isbn: Some(isbn.to_string()),
        title: work.title,
        publisher: edition.publishers.into_iter().next(),
        authors,
        language: edition
            .languages
            .into_iter()
            .next()
            .and_then(|v| v.key.strip_prefix("/languages/").map(|v| v.to_string())),
        summary: work.description.map(|d| d.text()),
        tags: work.subjects,
        published,
        page_count: edition.number_of_pages,
        amazon_id: None,
        google_id: None,
        librarything_id: None,
        owned: false,
        read: false,
        covert_art_b64,
        series: None,
    }))
}

#[cfg(test)]
mod test {}
