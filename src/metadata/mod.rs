use chrono::NaiveDate;

use crate::Config;

mod calibre;
mod openlibrary;

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
    #[error("Could not fetch metadata with open library")]
    OpenLibrary(#[from] openlibrary::OpenLibraryMetadataError),
}

#[derive(serde::Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum MetadataProvider {
    Calibre,
    OpenLibrary,
}

pub async fn fetch_metadata(
    config: &Config,
    isbn: &str,
    provider: MetadataProvider,
) -> Result<Option<NullableBookDetails>, MetadataError> {
    match provider {
        MetadataProvider::Calibre => Ok(calibre::fetch_metadata(
            config
                .metadata
                .calibre
                .as_ref()
                .expect("missing calibre configuration"),
            isbn,
        )
        .await?),
        MetadataProvider::OpenLibrary => Ok(openlibrary::fetch_metadata(
            config
                .metadata
                .open_library
                .as_ref()
                .expect("missing open_library configuration"),
            isbn,
        )
        .await?),
    }
}
