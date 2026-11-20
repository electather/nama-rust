use std::{fs, sync::Arc};

use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, PointKey, Registrar, metadata::SourceManifest,
    recogniser::Recogniser,
};

use super::super::tests::Fixture;
use crate::{
    library::{Library, LibraryError, LibraryKind, Order},
    plugins::Registry,
};

const READER: &str = "reader";
const PLAIN: &str = "plain";

/// A Plugin that only says `.0` of itself, so it can be known without being enabled.
struct Described(Manifest);

impl Plugin for Described {
    fn manifest(&self) -> Manifest {
        self.0.clone()
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}

fn manifest(id: &str, source: Option<SourceManifest>, points: Vec<PointKey>) -> Manifest {
    Manifest {
        version: "0.0.0".to_owned(),
        extension_points: points,
        title_source: source,
        ..Manifest::new(PluginId::from(id.to_owned()))
    }
}

fn source(namespace: &str, searches: bool, looks_up: &[&str]) -> SourceManifest {
    SourceManifest {
        namespace: namespace.to_owned(),
        looks_up: looks_up.iter().map(|&n| n.to_owned()).collect(),
        searches,
    }
}

/// A Movies Library on a Scanner whose Plugins, none enabled, are TMDB, searching and looking
/// IMDb ids up, fanart, not searching, `READER`, a Recogniser, and `PLAIN`, which isn't one; and
/// a folder no Library has.
async fn fixture() -> (Fixture, String) {
    let plugins = Registry::new();
    let recogniser = PointKey::provide::<dyn Recogniser>();
    for described in [
        manifest("tmdb", Some(source("tmdb", true, &["imdb"])), Vec::new()),
        manifest("fanart", Some(source("fanart", false, &[])), Vec::new()),
        manifest(READER, None, vec![recogniser]),
        manifest(PLAIN, None, Vec::new()),
    ] {
        plugins.describe_builtin(move || Arc::new(Described(described.clone())));
    }
    let media = Fixture::with_plugins(LibraryKind::Movies, plugins).await;
    let other = media.root.with_file_name("other");
    fs::create_dir(&other).unwrap();
    let other = other.to_str().unwrap().to_owned();
    (media, other)
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

fn sources(namespaces: &[&str]) -> Order {
    Order {
        title_sources: names(namespaces),
        ..Order::default()
    }
}

fn recognisers(ids: &[&str]) -> Order {
    Order {
        recognisers: Some(names(ids)),
        ..Order::default()
    }
}

impl Fixture {
    async fn create(&self, folder: &str, order: Order) -> Result<Library, LibraryError> {
        let (name, folders) = ("Other".to_owned(), vec![folder.to_owned()]);
        (self
            .scanner
            .create_library(name, LibraryKind::Movies, folders, order))
        .await
    }

    async fn update(&self, order: Order) -> Result<Library, LibraryError> {
        let folders = vec![self.root.to_str().unwrap().to_owned()];
        let name = "Media".to_owned();
        (self
            .scanner
            .update_library(self.library, name, folders, order))
        .await
    }

    async fn stored(&self) -> Library {
        self.scanner
            .inner
            .libraries
            .get(self.library)
            .await
            .unwrap()
    }

    async fn library_count(&self) -> usize {
        self.scanner.inner.libraries.list().await.unwrap().len()
    }
}

#[tokio::test]
async fn a_library_created_naming_no_title_source_gets_the_default() {
    let (media, other) = fixture().await;

    let created = media.create(&other, Order::default()).await.unwrap();

    assert_eq!(created.title_sources, names(&["tmdb"]));
    let libraries = &media.scanner.inner.libraries;
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, names(&["tmdb"]));
}

#[tokio::test]
async fn a_malformed_or_repeated_title_source_is_refused_naming_it() {
    let (media, other) = fixture().await;

    let malformed = media.create(&other, sources(&["TMDB"])).await;
    let repeated = media.update(sources(&["tmdb", "fanart", "tmdb"])).await;

    assert!(
        matches!(&malformed, Err(LibraryError::TitleSources { namespace: Some(n) }) if n == "TMDB"),
        "{malformed:?}"
    );
    assert!(
        matches!(&repeated, Err(LibraryError::TitleSources { namespace: Some(n) }) if n == "tmdb"),
        "{repeated:?}"
    );
    assert_eq!(media.library_count().await, 1);
    assert_eq!(media.stored().await.title_sources, names(&["tmdb"]));
}

#[tokio::test]
async fn a_title_source_no_plugin_keeps_or_looks_up_is_refused_naming_it() {
    let (media, other) = fixture().await;

    let created = media.create(&other, sources(&["tmdb", "gone"])).await;
    let updated = media.update(sources(&["gone", "tmdb"])).await;
    let looked_up = media.update(sources(&["imdb", "tmdb"])).await.unwrap();

    for refused in [created.map(drop), updated.map(drop)] {
        assert!(
            matches!(&refused, Err(LibraryError::TitleSourceUnknown { namespace }) if namespace == "gone"),
            "{refused:?}"
        );
    }
    assert_eq!(looked_up.title_sources, names(&["imdb", "tmdb"]));
    assert_eq!(media.library_count().await, 1);
}

#[tokio::test]
async fn an_order_with_no_source_that_searches_is_refused() {
    let (media, other) = fixture().await;

    let created = media.create(&other, sources(&["fanart"])).await;
    let updated = media.update(sources(&["fanart"])).await;

    assert!(matches!(created, Err(LibraryError::NoTitleSourceSearches)));
    assert!(matches!(updated, Err(LibraryError::NoTitleSourceSearches)));
    assert_eq!(media.stored().await.title_sources, names(&["tmdb"]));
}

#[tokio::test]
async fn an_update_leaves_a_stored_namespace_whose_plugin_is_gone_unjudged() {
    let (media, _other) = fixture().await;
    let libraries = &media.scanner.inner.libraries;
    (libraries.set_title_sources(media.library, names(&["gone", "fanart"])))
        .await
        .unwrap();

    let reordered = media.update(sources(&["fanart", "gone"])).await.unwrap();

    assert_eq!(reordered.title_sources, names(&["fanart", "gone"]));
    assert_eq!(
        media.stored().await.title_sources,
        names(&["fanart", "gone"])
    );
}

#[tokio::test]
async fn an_unchanged_order_is_accepted_without_being_judged() {
    let (media, _other) = fixture().await;
    let libraries = &media.scanner.inner.libraries;
    (libraries.set_title_sources(media.library, names(&["fanart"])))
        .await
        .unwrap();

    let kept = media.update(sources(&["fanart"])).await.unwrap();

    assert_eq!(kept.title_sources, names(&["fanart"]));
}

#[tokio::test]
async fn a_bad_recogniser_order_is_refused_naming_the_plugin() {
    let (media, other) = fixture().await;
    let bad: [(&[&str], &str); 4] = [
        (&[READER, READER], READER),
        (&["a,b"], "a,b"),
        (&[PLAIN], PLAIN),
        (&["missing"], "missing"),
    ];

    for (ids, plugin) in bad {
        let created = media.create(&other, recognisers(ids)).await;
        let updated = media.update(recognisers(ids)).await;

        for refused in [created, updated] {
            assert!(
                matches!(&refused, Err(LibraryError::RecognisersInvalid { plugin: p }) if p == plugin),
                "{ids:?}: {refused:?}"
            );
        }
    }
    assert_eq!(media.library_count().await, 1);
    assert_eq!(media.stored().await.recognisers, Vec::<String>::new());
}

#[tokio::test]
async fn a_disabled_recogniser_plugin_is_accepted() {
    let (media, other) = fixture().await;

    let created = media.create(&other, recognisers(&[READER])).await.unwrap();
    let updated = media.update(recognisers(&[READER])).await.unwrap();

    assert_eq!(created.recognisers, names(&[READER]));
    assert_eq!(updated.recognisers, names(&[READER]));
    assert_eq!(media.stored().await.recognisers, names(&[READER]));
}
