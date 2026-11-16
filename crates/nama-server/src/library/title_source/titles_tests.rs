use nama_plugin_api::{
    contract as wire,
    metadata::{ExternalId, MetadataError},
};

use super::{
    Artwork, ArtworkKind, SearchQuery, TitleKind, artwork, candidate, episode, search_query,
    season, show,
};

fn id(namespace: &str, value: &str) -> wire::TitleId {
    wire::TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

#[test]
fn a_candidate_becomes_the_kernels_with_every_id() {
    let found = wire::Candidate {
        id: id("tvdb", "81189").into(),
        ids: vec![id("tvdb", "81189"), id("imdb", "tt0903747")],
        title: "Breaking Bad".to_owned(),
        year: Some(2008),
        origin_country: vec!["US".to_owned()],
        vote_count: 12,
        ..Default::default()
    };

    let candidate = candidate(found).unwrap();

    assert_eq!(candidate.id, ExternalId::tvdb(81189));
    assert_eq!(
        candidate
            .ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
        ["imdb-tt0903747", "tvdb-81189"]
    );
    assert_eq!(
        (
            candidate.title.as_str(),
            candidate.year,
            candidate.vote_count
        ),
        ("Breaking Bad", Some(2008), 12)
    );
}

#[test]
fn an_answer_with_a_malformed_id_fails_naming_it() {
    let bad_candidate = wire::Candidate {
        id: id("TVDB", "81189").into(),
        ..Default::default()
    };
    let no_id = wire::Candidate {
        title: "Breaking Bad".to_owned(),
        ..Default::default()
    };
    let bad_show = wire::Show {
        ids: vec![id("tvdb", "")],
        ..Default::default()
    };
    let bad_episode = wire::Season {
        episodes: vec![wire::Episode {
            ids: vec![id("t v", "1")],
            ..Default::default()
        }],
        ..Default::default()
    };

    let failures = [
        candidate(bad_candidate).err(),
        candidate(no_id).err(),
        show(bad_show).err(),
        season(bad_episode).err(),
    ]
    .map(|failed| match failed {
        Some(MetadataError::Failed(err)) => err.to_string(),
        other => panic!("{other:?}"),
    });

    assert_eq!(
        failures,
        [
            r#"the title source answered with the id "TVDB-81189": the namespace "TVDB" isn't one or more of a-z and 0-9"#,
            r#"the title source found "Breaking Bad" without an id"#,
            r#"the title source answered with the id "tvdb-": the id's value is empty"#,
            r#"the title source answered with the id "t v-1": the namespace "t v" isn't one or more of a-z and 0-9"#,
        ]
    );
}

#[test]
fn an_episode_keeps_its_numbers_and_still() {
    let aired = wire::Episode {
        ids: vec![id("tvdb", "349232")],
        season: 1,
        number: 2,
        title: "Cat's in the Bag...".to_owned(),
        runtime_minutes: Some(48),
        still: Some("/still.jpg".to_owned()),
        ..Default::default()
    };

    let episode = episode(aired).unwrap();

    assert_eq!(
        (episode.season, episode.number, episode.runtime_minutes),
        (1, 2, Some(48))
    );
    assert_eq!(episode.still.as_deref(), Some("/still.jpg"));
}

#[test]
fn a_search_asks_for_its_kind() {
    let query = SearchQuery {
        kind: TitleKind::Show,
        title: "Breaking Bad".to_owned(),
        year: None,
    };

    let asked = search_query(&query);

    assert_eq!(
        asked.kind.as_known(),
        Some(wire::TitleKind::TITLE_KIND_SHOW)
    );
    assert_eq!((asked.title.as_str(), asked.year), ("Breaking Bad", None));
}

#[test]
fn an_artwork_list_drops_only_its_malformed_items() {
    let art = |kind: wire::ArtworkKind, path: &str| wire::Artwork {
        kind: kind.into(),
        path: path.to_owned(),
        language: Some("en".to_owned()),
        width: Some(1000),
        rating: Some(5.5),
        votes: Some(3),
        ..Default::default()
    };
    let unrated = wire::Artwork {
        rating: Some(f64::NAN),
        ..art(wire::ArtworkKind::ARTWORK_KIND_BANNER, "/banner.png")
    };
    let list = vec![
        art(wire::ArtworkKind::ARTWORK_KIND_LOGO, "/logo.png"),
        art(wire::ArtworkKind::ARTWORK_KIND_POSTER, ""),
        art(wire::ArtworkKind::ARTWORK_KIND_UNSPECIFIED, "/what.png"),
        unrated,
    ];

    let kernels = artwork(list);

    let logo = Artwork {
        kind: ArtworkKind::Logo,
        path: "/logo.png".to_owned(),
        language: Some("en".to_owned()),
        rating: Some(5.5),
        votes: Some(3),
    };
    let banner = Artwork {
        kind: ArtworkKind::Other,
        path: "/banner.png".to_owned(),
        rating: None,
        ..logo.clone()
    };
    assert_eq!(kernels, [logo, banner]);
}
