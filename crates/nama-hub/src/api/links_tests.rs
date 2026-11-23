use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use buffa::Message as _;
use connectrpc::ErrorCode;
use nama_hub_proto::nama::hub::v1::{Refusal, RefusalReason};

use super::{refused, trending_to_proto};
use crate::{accounts::RecentAuthenticationRequired, links::Error, viewing::TrendingTitle};

#[test]
fn a_link_from_a_sign_in_too_old_is_refused_saying_how_recent_one_must_be() {
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

#[test]
fn a_trending_movie_reaches_a_server_as_its_external_id_and_weight() {
    let trending = trending_to_proto(&TrendingTitle {
        external_id: "imdb-tt0133093".to_owned(),
        weight: 0.5,
    })
    .unwrap();

    let id = trending.id.as_option().unwrap();
    assert_eq!(
        (id.namespace.as_str(), id.value.as_str()),
        ("imdb", "tt0133093")
    );
    assert!((trending.weight - 0.5).abs() < f32::EPSILON);
}

#[test]
fn a_trending_id_without_a_namespace_is_left_out() {
    let trending = trending_to_proto(&TrendingTitle {
        external_id: "603".to_owned(),
        weight: 1.0,
    });

    assert!(trending.is_none());
}
