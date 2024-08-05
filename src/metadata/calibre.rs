use std::io::Read;

use base64::prelude::*;
use bstr::{BString, ByteSlice};

use crate::CalibreConfig;

use super::NullableBookDetails;

#[derive(Debug, thiserror::Error)]
pub enum CalibreMetadataError {
    #[error("Could not launch metadata fetcher")]
    Launch(#[source] std::io::Error),
    #[error("Response is not a valid utf-8 document")]
    InvalidResponse(#[from] std::str::Utf8Error),
    #[error("Response is not a valid xml document")]
    InvalidXmlResponse(#[from] roxmltree::Error),
    #[error("Response contains an invalid date")]
    InvalidDate(#[from] chrono::ParseError),
    #[error("Could not read the cover art")]
    CoverArt(#[source] std::io::Error),
    #[error("Fetcher failed to get the metadata")]
    FetchFailure { stdout: BString, stderr: BString },
}

fn parse_opf(
    document: &str,
    cover_art: &[u8],
) -> Result<Option<NullableBookDetails>, CalibreMetadataError> {
    let document = roxmltree::Document::parse(document)?;

    let Some(metadata) = document
        .root_element()
        .descendants()
        .find(|e| e.tag_name().name() == "metadata")
    else {
        return Ok(None);
    };

    let filter_tag =
        |name: &'static str| metadata.descendants().filter(move |e| e.has_tag_name(name));
    let filter_tag_opf_attr = |name: &'static str, attr: &'static str, val: &'static str| {
        metadata.descendants().filter(move |e| {
            e.has_tag_name(name) && e.attribute(("http://www.idpf.org/2007/opf", attr)) == Some(val)
        })
    };

    let find_tag = |name: &'static str| filter_tag(name).next();
    let find_str_tag =
        |name: &'static str| find_tag(name).and_then(|e| e.text().map(|s| s.to_owned()));

    let find_str_tag_opf_attr = |name: &'static str, attr: &'static str, val: &'static str| {
        filter_tag_opf_attr(name, attr, val)
            .next()
            .and_then(|e| e.text().map(|s| s.to_owned()))
    };

    let authors: Vec<_> = filter_tag_opf_attr("creator", "role", "aut")
        .filter_map(|e| e.text().map(|s| s.to_owned()))
        .collect();

    let tags: Vec<_> = filter_tag("subject")
        .filter_map(|e| e.text().map(|s| s.to_owned()))
        .collect();

    Ok(Some(NullableBookDetails {
        title: find_str_tag("title"),
        isbn: find_str_tag_opf_attr("identifier", "scheme", "ISBN"),
        authors,
        tags,
        summary: find_str_tag("description"),
        published: find_tag("date")
            .and_then(|e| e.text())
            .map(chrono::DateTime::parse_from_rfc3339)
            .transpose()?
            .map(|d| d.date_naive()),
        publisher: find_str_tag("publisher"),
        language: find_str_tag("language"),
        google_id: find_str_tag_opf_attr("identifier", "scheme", "GOOGLE"),
        amazon_id: find_str_tag_opf_attr("identifier", "scheme", "AMAZON"),
        // TODO: Find the correct scheme for it
        librarything_id: None,
        // TODO: Find if there is a property for this
        page_count: None,
        covert_art_b64: if cover_art.is_empty() {
            None
        } else {
            Some(BASE64_STANDARD.encode(cover_art))
        },
    }))
}

pub(super) async fn fetch_metadata(
    config: &CalibreConfig,
    isbn: &str,
) -> Result<Option<NullableBookDetails>, CalibreMetadataError> {
    tracing::debug!("Fetching metadata for isbn '{isbn}'");

    let mut tmp_file = tempfile::Builder::new()
        .suffix(".jpg")
        .tempfile()
        .map_err(CalibreMetadataError::CoverArt)?;

    let output = tokio::process::Command::new(&config.fetcher)
        .arg("--isbn")
        .arg(isbn)
        .arg("--opf")
        .arg("--cover")
        .arg(tmp_file.path())
        .output()
        .await
        .map_err(CalibreMetadataError::Launch)?;

    tracing::debug!("Stdout:\n{}", output.stdout.as_bstr());
    tracing::debug!("Stderr:\n{}", output.stderr.as_bstr());

    if !output.status.success() {
        return Err(CalibreMetadataError::FetchFailure {
            stderr: output.stderr.into(),
            stdout: output.stdout.into(),
        });
    }

    let image = tokio::task::block_in_place(|| -> Result<_, CalibreMetadataError> {
        let mut image = Vec::new();
        tmp_file
            .read_to_end(&mut image)
            .map_err(CalibreMetadataError::CoverArt)?;
        Ok(image)
    })?;

    parse_opf(std::str::from_utf8(&output.stdout)?, &image)
}

#[cfg(test)]
mod test {
    use expect_test::expect;

    #[test]
    fn hp() {
        let document = include_str!("../../tests/hp.opf");

        let actual = super::parse_opf(document, &[]).unwrap().unwrap();
        let expected = expect![[r#"
            NullableBookDetails {
                isbn: Some(
                    "9781526626585",
                ),
                title: Some(
                    "Harry Potter and the Philosopher's Stone: MinaLima Edition",
                ),
                authors: [
                    "J. K. Rowling",
                ],
                tags: [
                    "Fiction",
                    "General",
                    "Fantasy",
                    "Juvenile Fiction",
                    "Action & Adventure",
                    "Fantasy & Magic",
                ],
                summary: Some(
                    "<p>An irresistible new edition of <em>Harry Potter and the Philosopher's Stone</em> created with ultra-talented designers MinaLima, the design magicians behind the gorgeous visual graphic style of the Harry Potter and Fantastic Beasts films. J.K. Rowling's complete and unabridged text is accompanied by MinaLima's handsome colour illustrations on nearly every page, superb design, and eight exclusive interactive paper-engineered elements – including Harry's Hogwarts letter, the magical entrance to Diagon Alley, a sumptuous feast in the Great Hall of Hogwarts and more.  </p>\n<p>Designed and illustrated by the iconic house of MinaLima – best known for establishing the graphic design of the Harry Potter and Fantastic Beasts films – this is the perfect gift for Harry Potter fans and a beautiful addition to any collector's bookshelf, enticing readers of all ages to discover the Harry Potter novels all over again.</p>",
                ),
                published: Some(
                    2020-08-15,
                ),
                publisher: Some(
                    "BLOOMSBURY",
                ),
                language: Some(
                    "eng",
                ),
                google_id: Some(
                    "cmNSzQEACAAJ",
                ),
                amazon_id: Some(
                    "1526626586",
                ),
                librarything_id: None,
                page_count: None,
                covert_art_b64: None,
            }
        "#]];

        expected.assert_debug_eq(&actual)
    }
}
