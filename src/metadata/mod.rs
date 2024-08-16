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
    pub read: bool,
    pub owned: bool,
    pub covert_art_b64: Option<String>,
    pub series: Option<(String, i32)>,
}

#[derive(thiserror::Error, Debug)]
pub enum MetadataError {
    #[error("Could not scrap metadata with calibre")]
    Calibre(#[from] calibre::CalibreMetadataError),
    #[error("Could not fetch metadata with open library")]
    OpenLibrary(#[from] openlibrary::OpenLibraryMetadataError),
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum MetadataProvider {
    Calibre,
    OpenLibrary,
}

impl MetadataProvider {
    pub fn all() -> &'static [Self] {
        &[Self::Calibre, Self::OpenLibrary]
    }

    pub fn serialized(&self) -> &'static str {
        match self {
            MetadataProvider::Calibre => "Calibre",
            MetadataProvider::OpenLibrary => "OpenLibrary",
        }
    }
}

impl std::fmt::Display for MetadataProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetadataProvider::Calibre => write!(f, "Calibre"),
            MetadataProvider::OpenLibrary => write!(f, "Open Library"),
        }
    }
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
