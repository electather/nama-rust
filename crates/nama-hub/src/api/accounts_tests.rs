use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use buffa::Message as _;
use connectrpc::ErrorCode;
use nama_hub_proto::nama::hub::v1::{Refusal, RefusalReason};

use super::refused;
use crate::accounts::{Error, RecentAuthenticationRequired};

#[test]
fn a_sign_in_too_old_is_refused_saying_how_recent_one_must_be() {
    let err = refused(Error::RecentAuthenticationRequired(
        RecentAuthenticationRequired,
    ));

    assert_eq!(err.code, ErrorCode::FailedPrecondition);
    let value = err.details[0].value.as_deref().unwrap();
    let refusal = Refusal::decode_from_slice(&STANDARD_NO_PAD.decode(value).unwrap()).unwrap();
    assert_eq!(
        refusal.reason,
        RefusalReason::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED
    );
    assert_eq!(refusal.params["recentMinutes"], "10");
}
