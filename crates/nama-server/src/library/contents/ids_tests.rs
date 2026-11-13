use nama_plugin_api::metadata::{ExternalId, ExternalIds, IMDB};

use super::{parse_hint, primary_id};

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
fn the_primary_id_prefers_tmdb_then_imdb_then_tvdb_then_namespace_order() {
    let primary = |ids: &[ExternalId]| primary_id(&ids.iter().cloned().collect());
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
    assert_eq!(primary_id(&ExternalIds::default()), None);
}
