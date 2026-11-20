use std::sync::Arc;

use nama_plugin_api::metadata::{ExternalId, ExternalIds, IMDB};

use super::{DEFAULT_ORDER, Pin, matched_ids, parse_hint, primary_id};
use crate::{
    db::Database,
    library::{
        Metadata,
        fixture::{MatchedMovie, tmdb_movie},
        title_source::TitleKind,
    },
};

fn id(namespace: &str, value: &str) -> ExternalId {
    ExternalId::new(namespace, value).unwrap()
}

#[test]
fn a_stored_id_hint_reads_back_as_the_id_it_was() {
    let hints = [
        ExternalId::tmdb(78),
        ExternalId::tvdb(433_631),
        ExternalId::new(IMDB, "tt0083658").unwrap(),
        id("anidb", "123"),
        id("anidb", "a-b"),
    ];
    for hint in hints {
        assert_eq!(parse_hint(&hint.to_string()), Some(hint));
    }
}

#[test]
fn an_id_hint_needs_a_well_formed_namespace_and_a_value() {
    for text in ["Anidb-1", "-1", "anidb-", "anidb"] {
        assert_eq!(parse_hint(text), None, "{text}");
    }
}

#[test]
fn a_pinned_row_is_a_pin_on_its_id_or_on_unmatched_but_an_unparsable_id_is_no_pin() {
    assert_eq!(
        Pin::from_row(true, Some("tmdb-603")),
        Some(Pin::Title(ExternalId::tmdb(603)))
    );
    assert_eq!(Pin::from_row(true, None), Some(Pin::Unmatched));
    assert_eq!(Pin::from_row(true, Some("not an id")), None);
}

#[test]
fn the_primary_id_prefers_tmdb_then_imdb_then_tvdb_then_namespace_order() {
    let primary = |ids: &[ExternalId]| primary_id(&ids.iter().cloned().collect(), &DEFAULT_ORDER);
    let anidb = id("anidb", "1");
    let all = [
        anidb.clone(),
        ExternalId::tvdb(1),
        id(IMDB, "tt1"),
        ExternalId::tmdb(1),
    ];
    assert_eq!(primary(&all), Some(ExternalId::tmdb(1)));
    assert_eq!(primary(&all[..3]), Some(id(IMDB, "tt1")));
    assert_eq!(primary(&all[..2]), Some(ExternalId::tvdb(1)));
    assert_eq!(primary(&all[..1]), Some(anidb.clone()));
    assert_eq!(primary(&[id("mal", "1"), anidb.clone()]), Some(anidb));
    assert_eq!(primary_id(&ExternalIds::default(), &DEFAULT_ORDER), None);
}

#[test]
fn the_primary_id_follows_the_order_given_then_namespace_order() {
    let order = ["tvdb".to_owned(), "tmdb".to_owned()];
    let primary = |ids: &[ExternalId]| primary_id(&ids.iter().cloned().collect(), &order);
    let anidb = id("anidb", "1");
    let all = [
        anidb.clone(),
        ExternalId::tvdb(1),
        id(IMDB, "tt1"),
        ExternalId::tmdb(1),
    ];
    assert_eq!(primary(&all), Some(ExternalId::tvdb(1)));
    assert_eq!(
        primary(&[anidb.clone(), ExternalId::tmdb(1)]),
        Some(ExternalId::tmdb(1))
    );
    assert_eq!(primary(&[anidb.clone(), id(IMDB, "tt1")]), Some(anidb));
}

#[tokio::test]
async fn matched_ids_follow_matches_sharing_an_id_and_leave_other_titles() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(dir.path()).await.unwrap());
    let alien_imdb = id(IMDB, "tt0078748");
    let titles = [
        ("a", [ExternalId::tmdb(348), alien_imdb.clone()]),
        ("b", [alien_imdb.clone(), ExternalId::tvdb(1)]),
        ("c", [ExternalId::tmdb(949), id(IMDB, "tt0113277")]),
    ];
    for (folder, ids) in titles {
        let folder = dir.path().join(folder);
        std::fs::create_dir(&folder).unwrap();
        let matched = Metadata {
            ids: ExternalIds::from_iter(ids),
            ..tmdb_movie(0, "Alien")
        };
        MatchedMovie::new(&db, &folder, matched).await;
    }
    let mut conn = db.reader().acquire().await.unwrap();

    let mut found = (matched_ids(&mut conn, TitleKind::Movie, &ExternalId::tvdb(1)).await).unwrap();
    found.sort_by_key(ToString::to_string);
    let as_show = (matched_ids(&mut conn, TitleKind::Show, &ExternalId::tvdb(1)).await).unwrap();

    assert_eq!(
        found,
        [alien_imdb, ExternalId::tmdb(348), ExternalId::tvdb(1)]
    );
    assert_eq!(as_show, [ExternalId::tvdb(1)]);
}
