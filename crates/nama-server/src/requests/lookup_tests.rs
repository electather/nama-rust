use std::collections::HashSet;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::Request;

use crate::library::Candidate;

use super::{Found, movie_standing, ranked};

/// The Movie TMDB knows as `tmdb`, titled `title`, as a search of the title source finds it.
fn found(tmdb: i64, title: &str) -> Found<Candidate> {
    let id = ExternalId::tmdb(tmdb);
    Found {
        id: id.clone(),
        plugin: PluginId::from_static("tmdb"),
        movie: Candidate {
            id: id.clone(),
            ids: ExternalIds::from_iter([id]),
            title: title.to_owned(),
            original_title: None,
            year: None,
            poster: None,
            origin_country: Vec::new(),
            vote_count: 0,
        },
        library: None,
        request: None,
    }
}

fn titles(found: &[Found<Candidate>]) -> Vec<&str> {
    found
        .iter()
        .map(|found| found.movie.title.as_str())
        .collect()
}

#[test]
fn the_title_sources_movies_rank_available_then_requested_then_by_fit_then_in_its_order() {
    let in_library = Found {
        library: Some(1),
        ..found(4, "Planet of Aliens")
    };
    let requested = Found {
        request: Some(Request::default()),
        ..found(3, "The Alien Saga")
    };
    let source_order = vec![
        found(1, "Aliens"),
        found(2, "Alien"),
        requested,
        found(5, "Alien: Romulus"),
        in_library,
    ];

    let listed = ranked(source_order, &HashSet::new(), "alien", movie_standing);

    assert_eq!(
        titles(&listed),
        [
            "Planet of Aliens",
            "The Alien Saga",
            "Alien",
            "Aliens",
            "Alien: Romulus"
        ]
    );
}

#[test]
fn a_movie_the_librarys_batch_sent_under_any_of_its_ids_is_left_out() {
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let mut also_imdb = found(2, "Alien");
    also_imdb.movie.ids.insert(imdb.clone());
    let sent = HashSet::from([ExternalId::tmdb(1), imdb]);

    let listed = ranked(
        vec![found(1, "Aliens"), also_imdb, found(3, "Alien: Romulus")],
        &sent,
        "alien",
        movie_standing,
    );

    assert_eq!(titles(&listed), ["Alien: Romulus"]);
}
