use std::sync::Arc;

use nama_plugin_api::{
    contract::{EpisodeAsked, ImageAsked, SearchQuery, SeasonAsked, TitleAsked, TitleId},
    metadata::{ExternalId, MetadataError, TitleSource},
    settings::{SettingValue, Settings},
};

use super::super::{
    Running,
    bundled::BUNDLED,
    compile,
    load::{load, write_cache_config},
};
use crate::{
    library::title_source::{Error, MetadataSource},
    plugins::Registry,
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

#[tokio::test]
async fn tvdb_asked_for_a_show_by_a_tmdb_id_answers_unsupported_id_which_is_not_found() {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "tvdb.wasm")
        .unwrap();
    let tvdb = load(wasm.to_vec(), &cache_config).unwrap();
    let running = Running {
        package: Arc::clone(&tvdb.package),
        compiled: Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap()),
        secrets: Vec::new().into(),
        crashes: Arc::default(),
        exports: Arc::clone(&tvdb.exports),
    };
    let plugins = Registry::new();
    let api_key = SettingValue::Text("key".to_owned());
    let settings = Settings::new([("api_key".to_owned(), api_key)].into());
    plugins
        .enable_with_settings(Arc::new(tvdb), settings)
        .unwrap();
    let asked = TitleAsked {
        id: TitleId {
            namespace: "tmdb".to_owned(),
            value: "1399".to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };

    let answer = running.show(&asked).await.unwrap();
    let through_kernel = MetadataSource::new(plugins.extension_points())
        .show(&ExternalId::tmdb(1399))
        .await;

    assert!(
        answer.unsupported_id && answer.show.is_unset(),
        "{answer:?}"
    );
    assert_eq!(running.lookup_namespaces(), Vec::<String>::new());
    assert_eq!(through_kernel.unwrap_err(), Error::NotFound);
}
