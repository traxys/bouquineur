use crate::OpenLibraryConfig;

use super::NullableBookDetails;

#[derive(thiserror::Error, Debug)]
pub enum OpenLibraryMetadataError {}

pub(super) async fn fetch_metadata(
    _config: &OpenLibraryConfig,
    isbn: &str,
) -> Result<Option<NullableBookDetails>, OpenLibraryMetadataError> {
    tracing::debug!("Querying OpenLibrary for isbn '{isbn}'");

    Ok(None)
}

#[cfg(test)]
mod test {}
