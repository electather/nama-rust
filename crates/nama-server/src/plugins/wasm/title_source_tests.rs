use std::sync::Arc;

use nama_plugin_api::{
    contract::{
        ArtworkAsked, EpisodeAsked, ImageAsked, SearchQuery, SeasonAsked, TitleAsked, TitleId,
        TitleKind,
    },
    metadata::{ExternalId, MetadataError, TitleMethod, TitleSource},
    settings::{SettingValue, Settings},
};

use nama_plugin_proto::{
    nama::plugin::v1::{Manifest, Point},
    services,
};

use super::super::{
    Running,
    bundled::BUNDLED,
    check_exports, compile,
    load::{load, write_cache_config},
    points,
    services::TITLE_ARTWORK,
};
use crate::{
    library::title_source::{TitleSources, Unanswered},
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
        running.artwork(&ArtworkAsked::default()).await.err(),
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
async fn tvdb_asked_by_a_tmdb_id_answers_unsupported_id_which_is_not_found() {
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
    // Routing never asks TVDB by a tmdb id, so the Kernel asks it directly, as for a Season.
    let kernel = TitleSources::new(plugins.extension_points());
    let tvdb_source = kernel.sources().await.remove(0);
    let through_kernel = kernel
        .season_from(&tvdb_source, &ExternalId::tmdb(1399), 1)
        .await;

    assert!(
        answer.unsupported_id && answer.show.is_unset(),
        "{answer:?}"
    );
    assert_eq!(running.lookup_namespaces(), Vec::<String>::new());
    assert_eq!(through_kernel.unwrap_err(), Unanswered::NotFound);
}

#[tokio::test]
async fn a_title_source_exporting_only_artwork_and_image_loads_and_is_asked_for_artwork() {
    let package = Manifest {
        points: vec![Point {
            service: services::TITLE_SOURCE.to_owned(),
            version: "^1.1.0".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let exported = ["title_artwork", "title_image"];
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let (_, wasm) = BUNDLED
        .iter()
        .find(|(file, _)| *file == "radarr.wasm")
        .unwrap();
    let radarr = load(wasm.to_vec(), &cache_config).unwrap();
    // Radarr has no `title_artwork`, so a call reaching the export fails naming it, where a
    // method not served would answer unsupported without a call.
    let running = Running {
        package: Arc::clone(&radarr.package),
        compiled: Arc::new(compile(wasm, &cache_config, Vec::new(), Vec::new()).unwrap()),
        secrets: Vec::new().into(),
        crashes: Arc::default(),
        exports: Arc::new(exported.into_iter().collect()),
    };
    let asked = ArtworkAsked {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        id: TitleId {
            namespace: "tmdb".to_owned(),
            value: "603".to_owned(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };

    let checked = check_exports(&points(&package).unwrap(), |export| {
        exported.contains(&export)
    });
    let failed = match running.artwork(&asked).await {
        Err(MetadataError::Failed(failed)) => failed.to_string(),
        answer => format!("{answer:?}"),
    };

    assert!(checked.is_ok(), "{checked:?}");
    assert!(running.serves(&TITLE_ARTWORK));
    assert!(running.answers(TitleMethod::Artwork));
    assert!(!running.answers(TitleMethod::Movie));
    assert!(failed.contains("title_artwork"), "{failed}");
}
