use nama_plugin_api::{
    contract::{ArtworkAsked, TitleId, TitleKind},
    metadata::TitleSource,
};

use super::Client;
use crate::Endpoints;

/// A client whose TMDB isn't there, so any request it sends fails.
fn unreachable() -> Client {
    let endpoints = Endpoints {
        api: "http://127.0.0.1:9/3".to_owned(),
        images: "http://127.0.0.1:9/t/p".to_owned(),
    };
    Client::new(endpoints, "key", "en-US").unwrap()
}

fn asked(namespace: &str, value: &str) -> ArtworkAsked {
    ArtworkAsked {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        id: TitleId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn artwork_by_a_foreign_namespace_answers_unsupported_id_without_asking_tmdb() {
    let answer = unreachable()
        .artwork(&asked("fanart", "348"))
        .await
        .unwrap();

    assert!(answer.unsupported_id);
    assert_eq!(answer.artwork, []);
}

#[tokio::test]
async fn artwork_by_a_malformed_id_is_empty_without_asking_tmdb() {
    let answer = unreachable()
        .artwork(&asked("imdb", "not-an-id"))
        .await
        .unwrap();

    assert!(!answer.unsupported_id);
    assert_eq!(answer.artwork, []);
}
