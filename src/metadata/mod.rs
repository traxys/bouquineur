use chrono::NaiveDate;

use crate::Config;

mod calibre;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct NullableBookDetails {
    pub isbn: Option<String>,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub summary: Option<String>,
    pub published: Option<NaiveDate>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub google_id: Option<String>,
    pub amazon_id: Option<String>,
    pub librarything_id: Option<String>,
    pub page_count: Option<i32>,
    pub covert_art_b64: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum MetadataError {
    #[error("Could not scrap metadata with calibre")]
    Calibre(#[from] calibre::CalibreMetadataError),
}

pub enum MetadataProvider {
    Calibre,
}

pub async fn fetch_metadata(
    config: &Config,
    isbn: &str,
    provider: MetadataProvider,
) -> Result<Option<NullableBookDetails>, MetadataError> {
    match provider {
        MetadataProvider::Calibre => Ok(calibre::fetch_metadata(config, isbn).await?),
    }
}
