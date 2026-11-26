use std::sync::Arc;

use nama_plugin_api::metadata::ExternalIds;

use super::{Sourced, Unavailable};
use crate::{
    library::TitleKind,
    plugins::ExtensionPoints,
    requests::testing::{ada_and_bob, followed_as_bob_is_capped, requests_on},
};

#[tokio::test]
async fn a_capped_viewer_sees_a_sourced_show_requested_only_if_theirs_or_its_rating_passes() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    let requestable = ["tmdb".to_owned()];

    let seen = followed_as_bob_is_capped(&db, async |bob, show| {
        let title = Sourced {
            kind: TitleKind::Show,
            ids: ExternalIds::from_iter([show.clone()]),
            title: "Show".to_owned(),
            year: None,
            poster: None,
        };
        let seen = requests.unavailable(bob, &title, &requestable).await;
        matches!(seen.unwrap(), Some(Unavailable::Requested { .. }))
    })
    .await;

    // Ada's TV-MA, TV-PG and unrated Shows, then Bob's TV-MA one; else Requestable.
    let capped = [false, true, false, true];
    let capped_allowing_unrated = [false, true, true, true];
    assert_eq!(seen, [[true; 4], capped, capped_allowing_unrated]);
}
