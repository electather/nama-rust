use nama_plugin_api::PluginId;

use super::{Picked, pick};
use crate::library::{
    contents::ImageRef,
    title_source::{self, ArtworkKind, Source},
};

/// The source of the Plugin `plugin`.
fn source(plugin: &'static str) -> Source {
    Source {
        plugin: PluginId::from_static(plugin),
        namespace: plugin.to_owned(),
        looks_up: Vec::new(),
        answers: Vec::new(),
    }
}

/// A poster at `path`, in `language`, rated `rating` by `votes` people.
fn poster(
    path: &str,
    language: Option<&str>,
    rating: Option<f64>,
    votes: Option<u32>,
) -> title_source::Artwork {
    title_source::Artwork {
        kind: ArtworkKind::Poster,
        path: path.to_owned(),
        language: language.map(str::to_owned),
        rating,
        votes,
    }
}

/// The path of the poster [`pick`] picks from `lists`, in English.
fn picked_poster(lists: &[(Source, Vec<title_source::Artwork>)]) -> Option<String> {
    pick(lists, "en").poster.map(|poster| poster.path)
}

#[test]
fn the_language_asked_for_beats_the_source_order() {
    let lists = [
        (
            source("tmdb"),
            vec![poster("/fr.jpg", Some("fr"), Some(9.0), None)],
        ),
        (
            source("fanart"),
            vec![poster("/en.jpg", Some("en"), None, None)],
        ),
    ];
    let textless = [
        (
            source("tmdb"),
            vec![poster("/fr.jpg", Some("fr"), None, None)],
        ),
        (
            source("fanart"),
            vec![poster("/none.jpg", None, None, None)],
        ),
    ];

    assert_eq!(picked_poster(&lists).as_deref(), Some("/en.jpg"));
    assert_eq!(picked_poster(&textless).as_deref(), Some("/none.jpg"));
}

#[test]
fn the_source_order_beats_the_rating() {
    let lists = [
        (
            source("tmdb"),
            vec![poster("/low.jpg", Some("en"), Some(1.0), None)],
        ),
        (
            source("fanart"),
            vec![poster("/high.jpg", Some("en"), Some(9.0), None)],
        ),
    ];

    assert_eq!(picked_poster(&lists).as_deref(), Some("/low.jpg"));
}

#[test]
fn the_rating_then_the_votes_then_the_sources_own_order_decide() {
    let rated = [(
        source("tmdb"),
        vec![
            poster("/unrated.jpg", Some("en"), None, Some(90)),
            poster("/low.jpg", Some("en"), Some(5.0), Some(90)),
            poster("/high.jpg", Some("en"), Some(8.0), Some(1)),
        ],
    )];
    let voted = [(
        source("tmdb"),
        vec![
            poster("/few.jpg", Some("en"), Some(8.0), Some(1)),
            poster("/many.jpg", Some("en"), Some(8.0), Some(40)),
            poster("/many-too.jpg", Some("en"), Some(8.0), Some(40)),
        ],
    )];

    assert_eq!(picked_poster(&rated).as_deref(), Some("/high.jpg"));
    assert_eq!(picked_poster(&voted).as_deref(), Some("/many.jpg"));
}

#[test]
fn each_kind_is_picked_from_its_own_sources_list_and_names_its_plugin() {
    let logo = title_source::Artwork {
        kind: ArtworkKind::Logo,
        ..poster("/logo.png", Some("en"), None, None)
    };
    let clear_art = title_source::Artwork {
        kind: ArtworkKind::Other,
        ..poster("/clear.png", Some("en"), None, None)
    };
    let lists = [
        (
            source("tmdb"),
            vec![poster("/poster.jpg", None, None, None)],
        ),
        (source("fanart"), vec![logo, clear_art]),
    ];

    let picked = pick(&lists, "en");

    let image = |plugin, path: &str| {
        Some(ImageRef {
            plugin: PluginId::from_static(plugin),
            path: path.to_owned(),
        })
    };
    assert_eq!(
        picked,
        Picked {
            poster: image("tmdb", "/poster.jpg"),
            backdrop: None,
            titled_backdrop: None,
            logo: image("fanart", "/logo.png"),
        }
    );
}

/// A backdrop at `path`, in `language`, rated `rating`.
fn backdrop(path: &str, language: Option<&str>, rating: Option<f64>) -> title_source::Artwork {
    title_source::Artwork {
        kind: ArtworkKind::Backdrop,
        ..poster(path, language, rating, None)
    }
}

/// The path of the backdrop [`pick`] picks from `lists`, in English.
fn picked_backdrop(lists: &[(Source, Vec<title_source::Artwork>)]) -> Option<String> {
    pick(lists, "en").backdrop.map(|backdrop| backdrop.path)
}

#[test]
fn a_backdrop_without_text_beats_one_in_the_language_asked_for() {
    let lists = [(
        source("tmdb"),
        vec![
            backdrop("/en.jpg", Some("en"), Some(9.0)),
            backdrop("/none.jpg", None, None),
        ],
    )];

    assert_eq!(picked_backdrop(&lists).as_deref(), Some("/none.jpg"));
}

#[test]
fn a_poster_in_the_language_asked_for_still_beats_one_without_text() {
    let lists = [(
        source("tmdb"),
        vec![
            poster("/none.jpg", None, Some(9.0), None),
            poster("/en.jpg", Some("en"), None, None),
        ],
    )];

    assert_eq!(picked_poster(&lists).as_deref(), Some("/en.jpg"));
}

#[test]
fn backdrops_in_other_languages_only_are_picked_by_source_then_rating() {
    let lists = [
        (
            source("tmdb"),
            vec![
                backdrop("/fr-low.jpg", Some("fr"), Some(1.0)),
                backdrop("/de-high.jpg", Some("de"), Some(9.0)),
            ],
        ),
        (
            source("fanart"),
            vec![backdrop("/fr-top.jpg", Some("fr"), Some(10.0))],
        ),
    ];
    let english = [
        (
            source("tmdb"),
            vec![backdrop("/fr.jpg", Some("fr"), Some(9.0))],
        ),
        (
            source("fanart"),
            vec![backdrop("/en.jpg", Some("en"), None)],
        ),
    ];

    assert_eq!(picked_backdrop(&lists).as_deref(), Some("/de-high.jpg"));
    assert_eq!(picked_backdrop(&english).as_deref(), Some("/en.jpg"));
}

/// The path of the titled Backdrop [`pick`] picks from `lists`, in English.
fn picked_titled_backdrop(lists: &[(Source, Vec<title_source::Artwork>)]) -> Option<String> {
    pick(lists, "en")
        .titled_backdrop
        .map(|backdrop| backdrop.path)
}

#[test]
fn the_titled_backdrop_is_the_one_in_the_language_asked_for_and_the_backdrop_stays_textless() {
    let lists = [(
        source("tmdb"),
        vec![
            backdrop("/none.jpg", None, None),
            backdrop("/en.jpg", Some("en"), None),
            backdrop("/fr.jpg", Some("fr"), Some(9.0)),
        ],
    )];

    assert_eq!(picked_titled_backdrop(&lists).as_deref(), Some("/en.jpg"));
    assert_eq!(picked_backdrop(&lists).as_deref(), Some("/none.jpg"));
}

#[test]
fn no_backdrop_in_the_language_asked_for_picks_no_titled_backdrop() {
    let lists = [(
        source("tmdb"),
        vec![
            backdrop("/none.jpg", None, None),
            backdrop("/fr.jpg", Some("fr"), Some(9.0)),
            poster("/en.jpg", Some("en"), None, None),
        ],
    )];

    assert_eq!(picked_titled_backdrop(&lists), None);
}

#[test]
fn titled_backdrops_rank_by_source_then_rating_then_votes() {
    let voted = |path, votes| title_source::Artwork {
        votes: Some(votes),
        ..backdrop(path, Some("en"), Some(8.0))
    };
    let by_source = [
        (
            source("tmdb"),
            vec![backdrop("/low.jpg", Some("en"), Some(1.0))],
        ),
        (
            source("fanart"),
            vec![backdrop("/high.jpg", Some("en"), Some(9.0))],
        ),
    ];
    let by_rating = [(
        source("tmdb"),
        vec![
            backdrop("/low.jpg", Some("en"), Some(1.0)),
            backdrop("/high.jpg", Some("en"), Some(9.0)),
        ],
    )];
    let by_votes = [(
        source("tmdb"),
        vec![voted("/few.jpg", 1), voted("/many.jpg", 40)],
    )];

    assert_eq!(
        picked_titled_backdrop(&by_source).as_deref(),
        Some("/low.jpg")
    );
    assert_eq!(
        picked_titled_backdrop(&by_rating).as_deref(),
        Some("/high.jpg")
    );
    assert_eq!(
        picked_titled_backdrop(&by_votes).as_deref(),
        Some("/many.jpg")
    );
}
