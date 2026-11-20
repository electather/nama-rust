use super::manifest;

#[test]
fn manifest_says_where_to_get_an_api_key_and_gives_its_links_and_licence() {
    let manifest = manifest();

    let setup = &manifest.setup_markdown;
    assert!(setup.contains("Settings > General"), "{setup}");
    assert_eq!(
        [manifest.homepage_url, manifest.source_url, manifest.license],
        [
            "https://radarr.video/",
            "https://github.com/electather/nama-rust/tree/main/plugins/radarr",
            "AGPL-3.0-only",
        ]
    );
}
