//! Checking a refused request the way a client sees it: its Connect code and its `nama.v1.Refusal`
//! detail (ADR 0019).

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use buffa::{Message, MessageName};
use connectrpc::{ConnectError, ErrorCode};
use nama_proto::nama::v1::{Refusal, RefusalReason};

/// Asserts that `result` was refused with `code` and `reason`, and that the detail's `debug` holds
/// the same Refusal as its `value`. Returns the Refusal, to check its `field` or `params`.
#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn assert_refused<T>(
    result: Result<T, ConnectError>,
    code: ErrorCode,
    reason: RefusalReason,
) -> Refusal {
    let err = result.err().expect("the request should have been refused");
    assert_eq!(err.code, code, "{err:?}");
    assert_eq!(
        err.details.len(),
        1,
        "a refusal carries one detail: {err:?}"
    );
    let detail = err.details.first().expect("there is a detail");
    assert_eq!(detail.type_url, Refusal::FULL_NAME, "{err:?}");
    let value = detail.value.as_deref().expect("a Refusal has a value");
    let bytes = STANDARD_NO_PAD
        .decode(value)
        .expect("the value is unpadded base64");
    let refusal = Refusal::decode_from_slice(&bytes).expect("the value is a Refusal");
    assert_eq!(refusal.reason, reason, "{err:?}");
    let debug = detail.debug.clone().expect("a Refusal has a debug form");
    let debug: Refusal = serde_json::from_value(debug).expect("the debug form is a Refusal");
    assert!(debug == refusal, "{debug:?} isn't {refusal:?}");
    refusal
}
