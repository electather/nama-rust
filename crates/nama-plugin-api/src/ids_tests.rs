use super::{ExternalId, ExternalIds, IMDB, InvalidId, TMDB, TVDB};

#[test]
fn new_refuses_a_malformed_namespace_or_an_empty_value() {
    for namespace in ["", "TMDB", "tm-db", "tmdb "] {
        assert_eq!(
            ExternalId::new(namespace, "1"),
            Err(InvalidId::Namespace(namespace.to_owned()))
        );
    }
    assert_eq!(ExternalId::new("tmdb", ""), Err(InvalidId::EmptyValue));
    let id = ExternalId::new("anidb2", "x").unwrap();
    assert_eq!((id.namespace(), id.value()), ("anidb2", "x"));
}

#[test]
fn an_id_reads_back_from_its_text() {
    let ids = [
        ExternalId::tmdb(603),
        ExternalId::new(IMDB, "tt0133093").unwrap(),
        ExternalId::new("example", "a-b-c").unwrap(),
    ];
    for id in ids {
        assert_eq!(id.to_string().parse::<ExternalId>(), Ok(id));
    }
    let id: ExternalId = "example-a-b".parse().unwrap();
    assert_eq!((id.namespace(), id.value()), ("example", "a-b"));
    assert_eq!("tmdb".parse::<ExternalId>(), Err(InvalidId::NoSeparator));
    assert_eq!("tmdb-".parse::<ExternalId>(), Err(InvalidId::EmptyValue));
}

#[test]
fn numeric_values_sort_as_numbers() {
    assert!(ExternalId::tmdb(9) < ExternalId::tmdb(10));
    assert!(ExternalId::new(IMDB, "tt9").unwrap() < ExternalId::tmdb(1));
}

#[test]
fn a_set_holds_one_id_per_well_known_namespace() {
    let imdb = ExternalId::new(IMDB, "tt0133093").unwrap();
    let mut ids: ExternalIds = [ExternalId::tmdb(1), imdb.clone(), ExternalId::tvdb(7)]
        .into_iter()
        .collect();
    ids.insert(ExternalId::tmdb(603));
    assert_eq!(ids.get(TMDB), Some(ExternalId::tmdb(603)));
    assert_eq!(ids.get(TVDB), Some(ExternalId::tvdb(7)));
    assert_eq!(ids.get("example"), None);
    assert_eq!(
        ids.iter().collect::<Vec<_>>(),
        [imdb, ExternalId::tmdb(603), ExternalId::tvdb(7)]
    );
    assert!(!ids.is_empty());
    assert!(ExternalIds::default().is_empty());
}

#[test]
fn a_set_holds_an_id_of_any_namespace_one_per_namespace() {
    let id = |namespace, value| ExternalId::new(namespace, value).unwrap();
    let mut ids: ExternalIds = [id("mal", "5"), ExternalId::tmdb(603), id("anidb", "1")]
        .into_iter()
        .collect();
    assert_eq!(ids.get("anidb"), Some(id("anidb", "1")));
    assert_eq!(ids.get("mal"), Some(id("mal", "5")));
    assert_eq!(ids.get(TMDB), Some(ExternalId::tmdb(603)));
    assert_eq!(
        ids.iter().collect::<Vec<_>>(),
        [id("anidb", "1"), id("mal", "5"), ExternalId::tmdb(603)]
    );

    ids.insert(id("anidb", "2"));
    assert_eq!(ids.get("anidb"), Some(id("anidb", "2")));
    assert_eq!(ids.iter().count(), 3);
}
