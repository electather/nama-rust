use nama_plugin_api::metadata::ExternalId;

#[test]
fn a_stored_id_hint_reads_back_as_the_id_it_was() {
    let hints = [
        ExternalId::Tmdb(78),
        ExternalId::Tvdb(433_631),
        ExternalId::Imdb("tt0083658".to_owned()),
    ];
    for hint in hints {
        assert_eq!(super::parse_hint(&hint.to_string()), Some(hint));
    }
}
