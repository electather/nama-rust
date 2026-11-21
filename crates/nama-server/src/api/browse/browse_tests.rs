use connectrpc::ErrorCode;
use nama_proto::nama::v1::{Refusal, RefusalReason};

use crate::{browse, library::Stored, page::TokenInvalid};

use super::{Fields, refused, version_to_proto};

#[test]
fn a_page_token_from_another_list_is_refused_naming_its_field() {
    let err = refused(Fields::LIBRARY)(browse::Error::PageTokenInvalid(TokenInvalid));

    let debug = err.details.first().and_then(|detail| detail.debug.clone());
    let refusal: Refusal = serde_json::from_value(debug.unwrap()).unwrap();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert_eq!(
        refusal.reason.as_known(),
        Some(RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID)
    );
    assert_eq!(refusal.field, "page.token");
}

#[test]
fn a_version_is_sent_with_its_edition_and_its_file_name_without_folders() {
    let version = browse::Version {
        file: Stored {
            id: 7,
            size: 10,
            modified_ns: 0,
            parser_version: 0,
        },
        path: "/media/Alien (1979)/Alien (1979) {edition-Director's Cut}.mkv".to_owned(),
        edition: Some("Director's Cut".to_owned()),
        tracks: Vec::new(),
    };

    let sent = version_to_proto(&version);

    assert_eq!(
        (sent.file_name.as_str(), sent.edition.as_deref()),
        (
            "Alien (1979) {edition-Director's Cut}.mkv",
            Some("Director's Cut")
        )
    );
}
