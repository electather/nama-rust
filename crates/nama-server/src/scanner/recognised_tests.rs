use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{FileRecognised, FileToRecognise, RecognisedEpisodes, RecognisedMovie, TitleId},
    metadata::ExternalId,
    recogniser::Recogniser,
};
use tokio_util::sync::CancellationToken;

use super::{super::tests::Fixture, Planned, items, listed, recognise};
use crate::{
    library::contents::{Change, Item},
    library::libraries::Kind,
    plugins::Registry,
};

fn movie(title: &str) -> FileRecognised {
    FileRecognised {
        recognised: RecognisedMovie {
            title: title.to_owned(),
            year: Some(1979),
            ids: vec![TitleId {
                namespace: "tmdb".to_owned(),
                value: "348".to_owned(),
                ..TitleId::default()
            }],
            ..RecognisedMovie::default()
        }
        .into(),
        ..FileRecognised::default()
    }
}

fn episodes(numbers: impl IntoIterator<Item = u32>) -> FileRecognised {
    FileRecognised {
        recognised: RecognisedEpisodes {
            show_title: "Show".to_owned(),
            show_year: Some(2017),
            season: 1,
            episodes: numbers.into_iter().collect(),
            ..RecognisedEpisodes::default()
        }
        .into(),
        ..FileRecognised::default()
    }
}

fn episode(number: i64) -> Item {
    Item::Episode {
        show: "Show".to_owned(),
        year: Some(2017),
        hint: None,
        season: 1,
        episode: number,
    }
}

/// A Recogniser answering every file with `.0`'s proposal, or failing where it gives none.
struct Proposing(fn() -> Option<FileRecognised>);

impl Recogniser for Proposing {
    fn recognise_file<'a>(
        &'a self,
        _file: &'a FileToRecognise,
    ) -> BoxFuture<'a, Result<FileRecognised, PluginError>> {
        let answer = self.0().ok_or_else(|| "the Recogniser is down".into());
        Box::pin(std::future::ready(answer))
    }
}

struct ProposingPlugin(fn() -> Option<FileRecognised>);

impl Plugin for ProposingPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Recogniser>()],
            ..Manifest::new(PluginId::from_static("proposing"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Recogniser>(Priority::DEFAULT, Arc::new(Proposing(self.0)));
        Ok(())
    }
}

/// What a scan of a Shows Library listing the Recognisers `listing` stores when a Recogniser
/// answering as `answer` does is enabled as `proposing`: an unread new file, a parsed one and a
/// removed one.
async fn recognised_listing(
    listing: &[&str],
    answer: fn() -> Option<FileRecognised>,
) -> Vec<Change> {
    let plugins = Registry::new();
    plugins.enable(Arc::new(ProposingPlugin(answer))).unwrap();
    let store = |path: &str, items: Vec<Item>| Change::Store {
        path: path.to_owned(),
        size: 1,
        modified_ns: 1,
        items,
    };
    let planned = vec![
        Planned {
            change: store("/media/Show - 2.mkv", Vec::new()),
            unread: Some(PathBuf::from("Show - 2.mkv")),
        },
        Planned {
            change: store("/media/Show/Show - S01E01.mkv", vec![episode(1)]),
            unread: None,
        },
        Planned {
            change: Change::Remove(7),
            unread: None,
        },
    ];
    let order: Vec<String> = listing.iter().map(|&id| id.to_owned()).collect();
    let cancel = CancellationToken::new();
    recognise(
        &plugins.extension_points(),
        Kind::Shows,
        &order,
        planned,
        &cancel,
    )
    .await
    .unwrap()
}

/// [`recognised_listing`] with the Library listing the Recogniser.
async fn recognised_with(answer: fn() -> Option<FileRecognised>) -> Vec<Change> {
    recognised_listing(&["proposing"], answer).await
}

/// The paths of the files `changes` stores, with their Items.
fn stored(changes: &[Change]) -> Vec<(&str, &[Item])> {
    changes
        .iter()
        .filter_map(|change| match change {
            Change::Store { path, items, .. } => Some((path.as_str(), items.as_slice())),
            Change::Reparse { .. } | Change::Remove(_) => None,
        })
        .collect()
}

#[test]
fn a_proposal_of_the_kind_the_library_holds_becomes_its_items() {
    let movie = items(Kind::Movies, movie("Alien"));
    let shows = items(Kind::Shows, episodes([3, 2]));

    assert_eq!(
        movie,
        Some(vec![Item::Movie {
            title: "Alien".to_owned(),
            year: Some(1979),
            hint: Some(ExternalId::tmdb(348)),
        }])
    );
    assert_eq!(shows, Some(vec![episode(2), episode(3)]));
}

#[test]
fn a_proposal_of_the_wrong_kind_is_ignored() {
    assert_eq!(items(Kind::Shows, movie("Alien")), None);
    assert_eq!(items(Kind::Movies, episodes([1])), None);
    assert_eq!(items(Kind::Shows, FileRecognised::default()), None);
}

#[test]
fn too_many_episodes_in_one_file_is_refused() {
    let most = items(Kind::Shows, episodes(1..=20)).map(|items| items.len());
    let too_many = items(Kind::Shows, episodes(1..=21));

    assert_eq!(most, Some(20));
    assert_eq!(too_many, None);
}

#[tokio::test]
async fn an_unread_file_takes_the_recognisers_proposal() {
    let changes = recognised_with(|| Some(episodes([2]))).await;

    assert_eq!(
        stored(&changes),
        [
            ("/media/Show - 2.mkv", &[episode(2)][..]),
            ("/media/Show/Show - S01E01.mkv", &[episode(1)][..]),
        ]
    );
    assert_eq!(changes.len(), 3);
}

#[tokio::test]
async fn a_recogniser_failure_drops_the_files_change_and_keeps_the_rest() {
    let changes = recognised_with(|| None).await;

    assert_eq!(
        stored(&changes),
        [("/media/Show/Show - S01E01.mkv", &[episode(1)][..])]
    );
    assert!(matches!(changes.last(), Some(Change::Remove(7))));
}

#[tokio::test]
async fn a_library_not_listing_the_recogniser_leaves_the_file_unread() {
    let changes = recognised_listing(&[], || Some(episodes([2]))).await;

    assert_eq!(
        stored(&changes),
        [
            ("/media/Show - 2.mkv", &[][..]),
            ("/media/Show/Show - S01E01.mkv", &[episode(1)][..]),
        ]
    );
}

#[test]
fn listed_keeps_the_librarys_order_of_enabled_recognisers_only() {
    let plugins = Registry::new();
    plugins.enable(Arc::new(ProposingPlugin(|| None))).unwrap();
    let order = ["gone", "proposing"].map(str::to_owned);

    let listed = listed(&plugins.extension_points(), &order);

    assert_eq!(listed, [PluginId::from_static("proposing")]);
}

/// A Recogniser that can't tell what any file is, counting the files it is asked about.
#[derive(Clone, Default)]
struct Counting(Arc<AtomicUsize>);

impl Recogniser for Counting {
    fn recognise_file<'a>(
        &'a self,
        _file: &'a FileToRecognise,
    ) -> BoxFuture<'a, Result<FileRecognised, PluginError>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(FileRecognised::default())))
    }
}

impl Plugin for Counting {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Recogniser>()],
            ..Manifest::new(PluginId::from_static("counting"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Recogniser>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

#[tokio::test]
async fn a_scan_asks_newly_listed_recognisers_about_unparsed_files_once() {
    let counting = Counting::default();
    let plugins = Registry::new();
    plugins.enable(Arc::new(counting.clone())).unwrap();
    let media = Fixture::with_plugins(Kind::Shows, plugins.extension_points()).await;
    media.write("Show - 2.mkv", 1);
    media.write("Show/Show - S01E01.mkv", 1);
    media.scan().await;
    let libraries = &media.scanner.inner.libraries;
    let library = libraries.list().await.unwrap()[0].id;
    let asked = || counting.0.load(Ordering::SeqCst);
    let unlisted = asked();

    libraries
        .set_recognisers(library, vec!["counting".to_owned()])
        .await
        .unwrap();
    let listed = media.scan().await;
    let asked_listed = asked();
    let again = media.scan().await;

    assert_eq!(
        (unlisted, listed.updated, asked_listed),
        (0, 1, 1),
        "listing it reads the unparsed file again"
    );
    assert_eq!((again.updated, asked()), (0, 1), "nothing changed since");
}
