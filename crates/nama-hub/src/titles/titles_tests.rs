use super::ExternalId;

#[test]
fn an_external_id_reads_from_namespace_value_text_only() {
    let id = |namespace: &str, value: &str| {
        Some(ExternalId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
        })
    };
    let longest_value = "6".repeat(ExternalId::MAX_LENGTH - "tmdb-".len());
    let longest = format!("tmdb-{longest_value}");
    let too_long = format!("{longest}6");
    let cases = [
        ("imdb-tt0133093", id("imdb", "tt0133093")),
        ("anidb-12-a", id("anidb", "12-a")),
        (longest.as_str(), id("tmdb", &longest_value)),
        ("603", None),
        ("-603", None),
        ("tmdb-", None),
        ("TMDB-603", None),
        ("tm db-603", None),
        (too_long.as_str(), None),
    ];

    for (text, expected) in cases {
        assert_eq!(ExternalId::parse(text), expected, "{text}");
    }
}
