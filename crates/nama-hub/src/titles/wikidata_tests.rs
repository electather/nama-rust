use super::{Fact, Title, parse_title};
use crate::titles::{ExternalId, TitleKind};

/// A line of the dump for item `id` with `claims`, each a property and its statements.
fn entity(id: &str, claims: &[(&str, &[String])]) -> String {
    let claims: Vec<String> = claims
        .iter()
        .map(|(property, statements)| format!(r#""{property}":[{}]"#, statements.join(",")))
        .collect();
    format!(
        r#"{{"type":"item","id":"{id}","labels":{{}},"claims":{{{}}}}},"#,
        claims.join(",")
    )
}

fn string(value: &str, rank: &str) -> String {
    format!(
        r#"{{"mainsnak":{{"snaktype":"value","datavalue":{{"value":"{value}","type":"string"}}}},"type":"statement","rank":"{rank}"}}"#
    )
}

fn class(id: &str) -> String {
    format!(
        r#"{{"mainsnak":{{"snaktype":"value","datavalue":{{"value":{{"entity-type":"item","id":"{id}"}},"type":"wikibase-entityid"}}}},"type":"statement","rank":"normal"}}"#
    )
}

/// A statement naming item `id`, ranked `rank`.
fn item(id: &str, rank: &str) -> String {
    class(id).replace(r#""rank":"normal""#, &format!(r#""rank":"{rank}""#))
}

fn fact(property: &'static str, item: &str) -> Fact {
    Fact {
        property,
        item: item.to_owned(),
    }
}

fn id(namespace: &str, value: &str) -> ExternalId {
    ExternalId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
    }
}

/// `parse_title` of `line`, which must parse.
fn parsed(line: &str) -> Option<Title> {
    let entity = line.trim_end_matches(',');
    parse_title(entity).unwrap_or_else(|err| panic!("{err}: {entity}"))
}

#[test]
fn a_film_keeps_its_imdb_and_tmdb_ids() {
    let line = entity(
        "Q83495",
        &[
            ("P31", &[class("Q11424")]),
            ("P345", &[string("tt0133093", "normal")]),
            ("P4947", &[string("603", "normal")]),
        ],
    );

    assert_eq!(
        parsed(&line),
        Some(Title {
            wikidata_id: "Q83495".to_owned(),
            kind: TitleKind::Movie,
            ids: vec![id("imdb", "tt0133093"), id("tmdb", "603")],
            facts: vec![],
        })
    );
}

#[test]
fn a_series_keeps_its_tmdb_series_and_tvdb_ids() {
    let line = entity(
        "Q1079",
        &[
            ("P31", &[class("Q5398426")]),
            ("P4983", &[string("1396", "normal")]),
            ("P4835", &[string("81189", "normal")]),
        ],
    );

    assert_eq!(
        parsed(&line),
        Some(Title {
            wikidata_id: "Q1079".to_owned(),
            kind: TitleKind::Show,
            ids: vec![id("tmdb", "1396"), id("tvdb", "81189")],
            facts: vec![],
        })
    );
}

#[test]
fn a_film_keeps_its_tvdb_movie_id() {
    let line = entity(
        "Q83495",
        &[
            ("P31", &[class("Q11424")]),
            ("P4947", &[string("603", "normal")]),
            ("P12196", &[string("169", "normal")]),
        ],
    );

    assert_eq!(
        parsed(&line).map(|title| title.ids),
        Some(vec![id("tmdb", "603"), id("tvdb", "169")])
    );
}

#[test]
fn a_tmdb_or_tvdb_id_says_the_kind_without_a_known_class() {
    let film = entity("Q1", &[("P4947", &[string("1", "normal")])]);
    let tvdb_film = entity("Q9", &[("P12196", &[string("9", "normal")])]);
    let series = entity("Q2", &[("P4835", &[string("2", "normal")])]);

    assert_eq!(
        parsed(&film).map(|title| title.kind),
        Some(TitleKind::Movie)
    );
    assert_eq!(
        parsed(&tvdb_film).map(|title| title.kind),
        Some(TitleKind::Movie)
    );
    assert_eq!(
        parsed(&series).map(|title| title.kind),
        Some(TitleKind::Show)
    );
}

#[test]
fn an_entity_neither_film_nor_series_is_left_out() {
    let person = entity(
        "Q25089",
        &[
            ("P31", &[class("Q5")]),
            ("P345", &[string("nm0000233", "normal")]),
        ],
    );

    assert_eq!(parsed(&person), None);
}

#[test]
fn an_entity_saying_both_film_and_series_is_left_out() {
    let line = entity(
        "Q3",
        &[
            ("P31", &[class("Q11424")]),
            ("P4983", &[string("3", "normal")]),
        ],
    );

    assert_eq!(parsed(&line), None);
}

#[test]
fn a_film_without_an_external_id_is_left_out() {
    let line = entity(
        "Q4",
        &[
            ("P31", &[class("Q11424")]),
            ("P345", &[string("", "normal")]),
        ],
    );

    assert_eq!(parsed(&line), None);
}

#[test]
fn the_preferred_id_wins_and_a_deprecated_one_never_counts() {
    let line = entity(
        "Q103569",
        &[
            ("P31", &[class("Q11424")]),
            (
                "P345",
                &[
                    string("tt9999999", "deprecated"),
                    string("tt0000001", "normal"),
                    string("tt0078748", "preferred"),
                ],
            ),
            ("P4947", &[string("348", "deprecated")]),
        ],
    );

    assert_eq!(
        parsed(&line).map(|title| title.ids),
        Some(vec![id("imdb", "tt0078748")])
    );
}

#[test]
fn an_unknown_value_is_skipped() {
    let unknown = r#"{"mainsnak":{"snaktype":"somevalue"},"type":"statement","rank":"preferred"}"#;
    let line = entity(
        "Q5",
        &[
            ("P31", &[class("Q11424")]),
            ("P345", &[unknown.to_owned(), string("tt0000005", "normal")]),
        ],
    );

    assert_eq!(
        parsed(&line).map(|title| title.ids),
        Some(vec![id("imdb", "tt0000005")])
    );
}

#[test]
fn an_entity_naming_no_id_property_is_skipped_unparsed() {
    assert_eq!(parse_title("not JSON at all").unwrap(), None);
}

#[test]
fn empty_claims_written_as_an_array_parse() {
    // Wikidata writes an empty object as `[]`.
    let line = r#"{"type":"item","id":"Q6","labels":{"en":{"value":"\"P345\""}},"claims":[]}"#;

    assert_eq!(parse_title(line).unwrap(), None);
}

#[test]
fn a_malformed_entity_naming_an_id_property_is_an_error() {
    parse_title(r#"{"id":"Q7","claims":{"P345":"#).unwrap_err();
}

#[test]
fn a_deprecated_id_of_the_other_kind_leaves_the_kind_alone() {
    let line = entity(
        "Q8",
        &[
            ("P31", &[class("Q11424")]),
            ("P4947", &[string("8", "normal")]),
            ("P4983", &[string("80", "deprecated")]),
        ],
    );

    assert_eq!(
        parsed(&line),
        Some(Title {
            wikidata_id: "Q8".to_owned(),
            kind: TitleKind::Movie,
            ids: vec![id("tmdb", "8")],
            facts: vec![],
        })
    );
}

#[test]
fn a_film_keeps_the_facts_its_vector_is_built_from() {
    let line = entity(
        "Q83495",
        &[
            ("P31", &[class("Q11424")]),
            ("P4947", &[string("603", "normal")]),
            (
                "P161",
                &[item("Q25089", "normal"), item("Q43416", "preferred")],
            ),
            ("P57", &[item("Q9545711", "normal")]),
            ("P58", &[item("Q9545711", "normal")]),
            ("P136", &[item("Q471839", "normal")]),
            ("P921", &[item("Q1137820", "normal")]),
            ("P166", &[item("Q103916", "normal")]),
            ("P8345", &[item("Q1129400", "normal")]),
            ("P179", &[item("Q1129400", "normal")]),
            ("P495", &[item("Q30", "normal")]),
            ("P364", &[item("Q1860", "normal")]),
        ],
    );

    assert_eq!(
        parsed(&line).map(|title| title.facts),
        Some(vec![
            fact("award", "Q103916"),
            fact("cast", "Q25089"),
            fact("cast", "Q43416"),
            fact("country", "Q30"),
            fact("director", "Q9545711"),
            fact("franchise", "Q1129400"),
            fact("genre", "Q471839"),
            fact("subject", "Q1137820"),
            fact("writer", "Q9545711"),
        ])
    );
}

#[test]
fn a_deprecated_or_unknown_fact_is_left_out() {
    let unknown = r#"{"mainsnak":{"snaktype":"somevalue"},"type":"statement","rank":"normal"}"#;
    let line = entity(
        "Q83495",
        &[
            ("P4947", &[string("603", "normal")]),
            (
                "P57",
                &[
                    item("Q1", "deprecated"),
                    unknown.to_owned(),
                    item("Q9545711", "normal"),
                ],
            ),
        ],
    );

    assert_eq!(
        parsed(&line).map(|title| title.facts),
        Some(vec![fact("director", "Q9545711")])
    );
}
