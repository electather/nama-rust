use std::sync::Arc;

use nama_plugin_api::{
    contract::{EpisodeAsked, ImageAsked, SearchQuery, SeasonAsked, TitleAsked, TitleId},
    metadata::{MetadataError, TitleSource},
};

use super::super::{
    Running,
    bundled::BUNDLED,
    compile,
    load::{load, write_cache_config},
};

#[tokio::test]
async fn a_title_source_without_its_exports_answers_unsupported_to_each_method() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "radarr.wasm")
        .unwrap();
    let radarr = load(wasm.to_vec(), &cache_config).unwrap();
    let mut package = (*radarr.package).clone();
    package.title_namespace = "tvdb".to_owned();
    // Radarr has no title export, so none of them may be called.
    let running = Running {
        package: Arc::new(package),
        compiled: Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap()),
        secrets: Vec::new().into(),
        crashes: Arc::default(),
        exports: Arc::clone(&radarr.exports),
    };
    let title = TitleId {
        namespace: "tvdb".to_owned(),
        value: "81189".to_owned(),
        ..Default::default()
    };
    let asked = TitleAsked {
        id: title.clone().into(),
        ..Default::default()
    };
    let season = SeasonAsked {
        show: title.clone().into(),
        season: 1,
        ..Default::default()
    };
    let episode = EpisodeAsked {
        show: title.into(),
        season: 1,
        episode: 1,
        ..Default::default()
    };
    let image = ImageAsked {
        path: "/poster.jpg".to_owned(),
        ..Default::default()
    };

    let answers = [
        running.search(&SearchQuery::default()).await.err(),
        running.movie(&asked).await.err(),
        running.show(&asked).await.err(),
        running.season(&season).await.err(),
        running.episode(&episode).await.err(),
        running.image(&image).await.err(),
    ];

    assert_eq!(running.namespace(), "tvdb");
    for answer in answers {
        assert!(
            matches!(answer, Some(MetadataError::Unsupported)),
            "{answer:?}"
        );
    }
}
