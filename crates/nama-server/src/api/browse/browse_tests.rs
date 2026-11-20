use connectrpc::ErrorCode;
use nama_proto::nama::v1::{Refusal, RefusalReason};

use crate::{browse, page::TokenInvalid};

use super::{Fields, refused};

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
