use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures_util::StreamExt;
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        FileRecognised, FileToRecognise, RecognisedEpisodes, RecognisedMovie, TitleId,
        file_recognised::Recognised,
    },
    metadata::ExternalId,
    recogniser::Recogniser,
};
use nama_proto::nama::v1::{EventType, ScanFinished, subscribe_response};
use tokio_util::sync::CancellationToken;

use super::{super::tests::Fixture, Planned, items, listed, recognise};
use crate::{
    events::{Filter, event},
    library::{Change, Item, LibraryKind},
    log_capture::CapturedLog,
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
            added: true,
        },
        Planned {
            change: store("/media/Show/Show - S01E01.mkv", vec![episode(1)]),
            unread: None,
            added: true,
        },
        Planned {
            change: Change::Remove(7),
            unread: None,
            added: false,
        },
    ];
    let order: Vec<String> = listing.iter().map(|&id| id.to_owned()).collect();
    let cancel = CancellationToken::new();
    recognise(
        &plugins.extension_points(),
        LibraryKind::Shows,
        &order,
        planned,
        &cancel,
    )
    .await
    .unwrap()
    .0
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
    let movie = items(LibraryKind::Movies, movie("Alien"));
    let shows = items(LibraryKind::Shows, episodes([3, 2]));

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
    assert_eq!(items(LibraryKind::Shows, movie("Alien")), None);
    assert_eq!(items(LibraryKind::Movies, episodes([1])), None);
    assert_eq!(items(LibraryKind::Shows, FileRecognised::default()), None);
}

/// The ID Hint Matching goes by for a Movie proposed with the one id `namespace`, `value`.
fn hint_of(namespace: &str, value: &str) -> Option<ExternalId> {
    let mut proposal = movie("Alien");
    if let Some(Recognised::Movie(movie)) = &mut proposal.recognised {
        movie.ids = vec![TitleId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..TitleId::default()
        }];
    }
    match items(LibraryKind::Movies, proposal)?.pop()? {
        Item::Movie { hint, .. } => hint,
        Item::Episode { .. } => None,
    }
}

#[test]
fn a_proposed_id_is_read_as_the_name_parser_reads_an_id_hint() {
    assert_eq!(hint_of("tmdb", "0123"), Some(ExternalId::tmdb(123)));
    assert_eq!(
        hint_of("imdbid", "TT0133093"),
        ExternalId::new("imdb", "tt0133093").ok()
    );
    assert_eq!(hint_of("TVDB", "12a"), None);
    assert_eq!(hint_of("edition", "Director's Cut"), None);
}

/// The year of the last Item proposed for `proposal` in a Library of `kind`.
fn year_of(kind: LibraryKind, proposal: FileRecognised) -> Option<i64> {
    match items(kind, proposal)?.pop()? {
        Item::Movie { year, .. } | Item::Episode { year, .. } => year,
    }
}

#[test]
fn a_proposed_year_no_name_could_give_is_dropped() {
    let mut movie = movie("Alien");
    if let Some(Recognised::Movie(proposed)) = &mut movie.recognised {
        proposed.year = Some(1066);
    }
    let mut show = episodes([1]);
    if let Some(Recognised::Episodes(proposed)) = &mut show.recognised {
        proposed.show_year = Some(1999);
    }

    assert_eq!(year_of(LibraryKind::Movies, movie), None);
    assert_eq!(year_of(LibraryKind::Shows, show), Some(1999));
}

#[test]
fn too_many_episodes_in_one_file_is_refused() {
    let most = items(LibraryKind::Shows, episodes(1..=20)).map(|items| items.len());
    let too_many = items(LibraryKind::Shows, episodes(1..=21));

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
    let media = Fixture::with_plugins(LibraryKind::Shows, plugins.clone()).await;
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

#[tokio::test]
async fn a_rescan_with_nothing_changed_finishes_while_other_writes_hold_the_writer() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;
    let finished = Filter {
        types: vec![EventType::EVENT_TYPE_SCAN_FINISHED],
        ..Filter::default()
    };
    let mut finished = Box::pin(media.events.subscribe_as_admin(finished));
    let (holding, held) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel::<()>();
    let db = Arc::clone(&media.db);
    let busy = tokio::spawn(async move {
        let held_until_released = async {
            holding.send(()).unwrap();
            released.await.map_err(sqlx::Error::decode)
        };
        db.write(|_| Box::pin(held_until_released)).await
    });
    held.await.unwrap();
    let scanner = media.scanner.clone();
    let library = media.library;
    let rescan = tokio::spawn(async move { scanner.scan(library).await });

    let published = tokio::time::timeout(Duration::from_secs(5), finished.next()).await;

    release.send(()).unwrap();
    busy.await.unwrap().unwrap();
    rescan.await.unwrap().unwrap();
    let expected = event(ScanFinished {
        library_id: media.library,
        unchanged: 1,
        ..Default::default()
    });
    assert_eq!(
        published
            .ok()
            .flatten()
            .and_then(|response| response.message),
        Some(subscribe_response::Message::from(expected)),
        "the rescan waited for the writer"
    );
}

#[tokio::test]
async fn a_scan_does_not_count_a_new_file_a_recogniser_failed_on_as_added() {
    let plugins = Registry::new();
    plugins.enable(Arc::new(ProposingPlugin(|| None))).unwrap();
    let media = Fixture::with_plugins(LibraryKind::Shows, plugins.clone()).await;
    let libraries = &media.scanner.inner.libraries;
    let library = libraries.list().await.unwrap()[0].id;
    libraries
        .set_recognisers(library, vec!["proposing".to_owned()])
        .await
        .unwrap();
    media.write("Show - 2.mkv", 1);
    media.write("Show/Show - S01E01.mkv", 1);

    let summary = media.scan().await;

    assert_eq!((summary.added, summary.updated), (1, 0));
}

#[tokio::test]
async fn a_recogniser_failing_on_several_files_is_counted_and_logged_once() {
    let plugins = Registry::new();
    plugins.enable(Arc::new(ProposingPlugin(|| None))).unwrap();
    let points = plugins.extension_points();
    let unread = |name: &str| Planned {
        change: Change::Store {
            path: format!("/media/{name}"),
            size: 1,
            modified_ns: 1,
            items: Vec::new(),
        },
        unread: Some(PathBuf::from(name)),
        added: true,
    };
    let planned = ["Show - 1.mkv", "Show - 2.mkv", "Show - 3.mkv"].map(unread);
    // The test's runtime has one thread, so the scan logs to it.
    let log = CapturedLog::start();

    let (_, dropped) = recognise(
        &points,
        LibraryKind::Shows,
        &["proposing".to_owned()],
        planned.into(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();

    let log = log.finish();
    assert_eq!(dropped.len(), 3);
    let failing = points
        .failing(
            &PluginId::from_static("proposing"),
            PointKey::provide::<dyn Recogniser>(),
        )
        .unwrap();
    assert_eq!(
        (failing.in_a_row, failing.last_error.as_str()),
        (3, "the Recogniser is down")
    );
    let warned: Vec<&str> = log.lines().filter(|line| line.contains("WARN")).collect();
    assert_eq!(warned.len(), 1, "{log}");
    assert!(warned[0].contains("files=3"), "{log}");
}

#[tokio::test]
async fn a_recogniser_answering_clears_its_failure() {
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(ProposingPlugin(|| Some(episodes([2])))))
        .unwrap();
    let points = plugins.extension_points();
    let (plugin, point) = (
        PluginId::from_static("proposing"),
        PointKey::provide::<dyn Recogniser>(),
    );
    points.provider_failed(&plugin, point, "the Recogniser is down");
    let planned = vec![Planned {
        change: Change::Store {
            path: "/media/Show - 2.mkv".to_owned(),
            size: 1,
            modified_ns: 1,
            items: Vec::new(),
        },
        unread: Some(PathBuf::from("Show - 2.mkv")),
        added: true,
    }];

    recognise(
        &points,
        LibraryKind::Shows,
        &["proposing".to_owned()],
        planned,
        &CancellationToken::new(),
    )
    .await
    .unwrap();

    assert_eq!(points.failing(&plugin, point), None);
}
