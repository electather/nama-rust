use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar, contract,
    fulfiller::{
        Download, EpisodeNumber, EpisodeNumbers, Fulfilled, Fulfiller, MovieDeleted, Movies,
        ShowDeleted, Shows, StopMonitoring, TitleId, TryAgainIn, show_deleted::Deleted,
        stop_monitoring::Title,
    },
    metadata::{ExternalId, ExternalIds},
};
use tokio::sync::Notify;

use super::ToDelete;
use crate::{
    library::{
        Change, Decided, Item, LibraryKind, Metadata, TitleTarget,
        fixture::{decide, tmdb_movie},
    },
    plugins::Registry,
    scanner::{Error, Scanner, tests::Fixture},
};

const SECRETS: i64 = 1_001;
const LIES: i64 = 1_002;
const PAST_AND_PRESENT: i64 = 1_003;

/// A Shows Library holding `Dark - S01E01-E02.mkv`, Matched to the Episodes Secrets and Lies, and
/// `Dark - S01E03.mkv`, Matched to Past and Present, with `plugins` as the Plugins the Scanner asks.
async fn dark(plugins: &Registry) -> Fixture {
    let media = Fixture::with_plugins(LibraryKind::Shows, plugins.clone()).await;
    store(
        &media,
        "Dark - S01E01-E02.mkv",
        &[(1, SECRETS, "Secrets"), (2, LIES, "Lies")],
    )
    .await;
    store(
        &media,
        "Dark - S01E03.mkv",
        &[(3, PAST_AND_PRESENT, "Past and Present")],
    )
    .await;
    media
}

/// Writes `file` in the Library and stores it holding each of `episodes` of Dark's Season 1, by
/// number, TMDB id and title, each Matched. A file stored before is stored again.
async fn store(media: &Fixture, file: &str, episodes: &[(i64, i64, &str)]) {
    media.write(file, 10);
    let path = media.path(file).to_str().unwrap().to_owned();
    let items = (episodes.iter())
        .map(|&(number, _, _)| Item::Episode {
            show: "Dark".to_owned(),
            year: None,
            hint: None,
            season: 1,
            episode: number,
        })
        .collect();
    let stored = Change::Store {
        path: path.clone(),
        size: 10,
        modified_ns: 0,
        items,
    };
    media
        .contents
        .apply(media.library, vec![stored])
        .await
        .unwrap();
    let decided = (episodes.iter())
        .map(|&(_, tmdb, title)| {
            Decided::Matched(Box::new(Metadata {
                show_ids: ExternalIds::from_iter([ExternalId::tmdb(70_523)]),
                show_title: Some("Dark".to_owned()),
                ..tmdb_movie(tmdb, title)
            }))
        })
        .collect();
    decide(&media.contents, media.library, &path, decided).await;
}

fn episode(media: &Fixture, tmdb: i64) -> ToDelete {
    ToDelete::Title {
        library: media.library,
        title: TitleTarget::Episode(ExternalId::tmdb(tmdb)),
    }
}

/// The names of the Episodes `error` refuses a delete for taking along unnamed.
fn unnamed(error: Error) -> Vec<String> {
    let Error::TakesAlongUnnamed(taken) = error else {
        panic!("refused for something else: {error}");
    };
    taken.iter().map(super::TakenAlong::name).collect()
}

/// Waits until `scanner` has a delete running.
async fn deleting(scanner: &Scanner) {
    for _ in 0..500 {
        if !scanner.inner.tasks.is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the delete never started");
}

/// A Plugin whose Fulfillers stop monitoring whatever they're told, and what they were told.
fn willing() -> (Registry, Arc<Busy>) {
    let told = Arc::new(Busy {
        willing: true,
        ..Busy::default()
    });
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(BusyPlugin(Arc::clone(&told))))
        .unwrap();
    (plugins, told)
}

fn tmdb(id: i64) -> Vec<TitleId> {
    vec![TitleId {
        namespace: "tmdb".to_owned(),
        value: id.to_string(),
        ..Default::default()
    }]
}

/// What deleting `deleted` of Dark tells a Fulfiller.
fn dark_deleted(deleted: impl Into<Deleted>) -> Title {
    Title::from(ShowDeleted {
        show_ids: tmdb(70_523),
        deleted: Some(deleted.into()),
        ..Default::default()
    })
}

/// Dark's Season 1 Episodes `numbers`, as a Fulfiller is told them.
fn dark_episodes(numbers: &[u32]) -> Title {
    let episodes = (numbers.iter())
        .map(|&episode_number| EpisodeNumber {
            season_number: 1,
            episode_number,
            ..Default::default()
        })
        .collect();
    dark_deleted(EpisodeNumbers {
        episodes,
        ..Default::default()
    })
}

#[tokio::test]
async fn deleting_episodes_or_a_season_stops_monitoring_those_and_the_ones_taken_along() {
    let (plugins, told) = willing();
    let media = dark(&plugins).await;
    let delete = |target, also| (media.scanner).delete_media(target, also, "ada".to_owned());

    delete(episode(&media, PAST_AND_PRESENT), Vec::new())
        .await
        .unwrap();
    delete(episode(&media, SECRETS), vec![ExternalId::tmdb(LIES)])
        .await
        .unwrap();
    store(
        &media,
        "Dark - S01E03.mkv",
        &[(3, PAST_AND_PRESENT, "Past and Present")],
    )
    .await;
    let season = TitleTarget::Season {
        show: ExternalId::tmdb(70_523),
        number: 1,
    };
    let season = ToDelete::Title {
        library: media.library,
        title: season,
    };
    delete(season, Vec::new()).await.unwrap();

    assert_eq!(
        *told.told.lock().unwrap(),
        [
            dark_episodes(&[3]),
            dark_episodes(&[1, 2]),
            dark_deleted(Deleted::SeasonNumber(1)),
        ]
    );
}

#[tokio::test]
async fn deleting_a_show_stops_monitoring_every_current_season_and_a_movie_or_its_version_the_movie()
 {
    let (plugins, told) = willing();
    let dark = dark(&plugins).await;
    let show = ToDelete::Title {
        library: dark.library,
        title: TitleTarget::Show(ExternalId::tmdb(70_523)),
    };
    let movies = Fixture::with_plugins(LibraryKind::Movies, plugins.clone()).await;
    for file in [
        "Alien (1979)/Alien (1979).mkv",
        "Alien (1979)/Alien (1979) - 2160p.mkv",
    ] {
        movies.write(file, 10);
    }
    movies.scan().await;
    movies
        .movie("Alien (1979)/Alien (1979) - 2160p.mkv", 348, "Alien")
        .await;
    let alien = movies
        .movie("Alien (1979)/Alien (1979).mkv", 348, "Alien")
        .await;
    let stored = movies.contents.stored(movies.library).await.unwrap();
    let one_version = ToDelete::Version(
        stored[movies
            .path("Alien (1979)/Alien (1979).mkv")
            .to_str()
            .unwrap()]
        .id,
    );

    for (scanner, target) in [
        (&dark.scanner, show),
        (&movies.scanner, one_version),
        (&movies.scanner, alien),
    ] {
        (scanner.delete_media(target, Vec::new(), "ada".to_owned()))
            .await
            .unwrap();
    }

    let told_of_alien = Title::from(MovieDeleted {
        ids: tmdb(348),
        ..Default::default()
    });
    assert_eq!(
        *told.told.lock().unwrap(),
        [
            dark_deleted(Deleted::EveryCurrentSeason(true)),
            told_of_alien.clone(),
            told_of_alien,
        ]
    );
}

#[tokio::test]
async fn a_preview_names_the_episodes_a_file_takes_along() {
    let media = dark(&Registry::new()).await;

    let planned = media
        .scanner
        .preview_delete(&episode(&media, SECRETS))
        .await;

    let planned = planned.unwrap();
    let taken: Vec<String> = planned
        .taken_along
        .iter()
        .map(super::TakenAlong::name)
        .collect();
    assert_eq!(taken, ["Dark S01E02"]);
    assert_eq!(planned.files.len(), 1);
}

#[tokio::test]
async fn a_delete_taking_along_an_episode_it_doesnt_name_is_refused_before_anything_is_asked() {
    let busy = Arc::new(Busy::default());
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(BusyPlugin(Arc::clone(&busy))))
        .unwrap();
    let media = dark(&plugins).await;

    let refused = (media.scanner)
        .delete_media(episode(&media, SECRETS), Vec::new(), "ada".to_owned())
        .await;

    assert_eq!(unnamed(refused.unwrap_err()), ["Dark S01E02"]);
    assert_eq!(busy.asked.load(Ordering::SeqCst), 0);
    assert!(media.path("Dark - S01E01-E02.mkv").exists());
}

#[tokio::test]
async fn a_delete_naming_the_episode_it_takes_along_deletes_the_file_and_both() {
    let media = dark(&Registry::new()).await;

    let deleted = (media.scanner)
        .delete_media(
            episode(&media, SECRETS),
            vec![ExternalId::tmdb(LIES)],
            "ada".to_owned(),
        )
        .await;

    assert_eq!(deleted.unwrap().files.len(), 1);
    assert!(!media.path("Dark - S01E01-E02.mkv").exists());
    let shows = media.contents.shows(media.library, None).await.unwrap();
    let left: Vec<i64> = shows[0].episodes().map(|episode| episode.number).collect();
    assert_eq!(left, [3]);
}

#[tokio::test]
async fn a_file_merged_while_the_delete_waited_for_the_scan_lock_refuses_it_and_leaves_every_file()
{
    let media = dark(&Registry::new()).await;
    let scan = media.scanner.lock(media.library).await;
    let scanner = media.scanner.clone();
    let delete = episode(&media, PAST_AND_PRESENT);
    let refused =
        tokio::spawn(
            async move { (scanner.delete_media(delete, Vec::new(), "ada".to_owned())).await },
        );
    deleting(&media.scanner).await;

    // As a scan finding the file renamed to hold Episode 4 too would store it.
    let both = [
        (3, PAST_AND_PRESENT, "Past and Present"),
        (4, 1_004, "Truths"),
    ];
    store(&media, "Dark - S01E03.mkv", &both).await;
    drop(scan);

    assert_eq!(
        unnamed(refused.await.unwrap().unwrap_err()),
        ["Dark S01E04"]
    );
    assert!(media.path("Dark - S01E03.mkv").exists());
    assert!(media.path("Dark - S01E01-E02.mkv").exists());
}

/// How long [`Busy`] says to wait.
const WAIT: Duration = Duration::from_millis(10);

/// The Plugin `busy`, whose Fulfillers say to try again in [`WAIT`] the first time
/// it is asked to stop monitoring a title, unless `willing`, counting each time it is asked and
/// recording what it was told.
#[derive(Default)]
struct Busy {
    willing: bool,
    told: std::sync::Mutex<Vec<Title>>,
    asked: AtomicUsize,
    /// Holds each ask after the first until notified. The retry is due [`WAIT`] after the first
    /// ask, and a slow delete can still be running then, so without it the count the test reads
    /// once the delete returns could already include the retry.
    again: Notify,
}

impl<K: 'static> Fulfiller<K> for Busy {
    fn fulfill<'a>(
        &'a self,
        _request: &'a contract::Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        Box::pin(async { Err("not asked here".into()) })
    }

    fn stop_monitoring<'a>(
        &'a self,
        title: &'a StopMonitoring,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async {
            if self.willing {
                self.told.lock().unwrap().extend(title.title.clone());
                return Ok(());
            }
            if self.asked.load(Ordering::SeqCst) > 0 {
                self.again.notified().await;
            }
            if self.asked.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(TryAgainIn(WAIT).into())
            } else {
                Ok(())
            }
        })
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// The Plugin providing `.0`.
struct BusyPlugin(Arc<Busy>);

impl Plugin for BusyPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![
                PointKey::provide::<dyn Fulfiller<Movies>>(),
                PointKey::provide::<dyn Fulfiller<Shows>>(),
            ],
            ..Manifest::new(PluginId::from_static("busy"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::<Busy>::clone(&self.0));
        registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, Arc::<Busy>::clone(&self.0));
        Ok(())
    }
}

/// Returns once `busy` was asked twice.
async fn asked_twice(busy: &Busy) {
    while busy.asked.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(WAIT).await;
    }
}

#[tokio::test]
async fn a_delete_whose_fulfiller_says_to_try_again_still_deletes_and_asks_again_later() {
    let busy = Arc::new(Busy::default());
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(BusyPlugin(Arc::clone(&busy))))
        .unwrap();
    let media = Fixture::with_plugins(LibraryKind::Movies, plugins.clone()).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;
    let alien = media
        .movie("Alien (1979)/Alien (1979).mkv", 348, "Alien")
        .await;

    let deleted = (media.scanner)
        .delete_media(alien, Vec::new(), "ada".to_owned())
        .await;
    let asked_by_delete = busy.asked.load(Ordering::SeqCst);
    busy.again.notify_one();
    let asked_again = tokio::time::timeout(Duration::from_secs(5), asked_twice(&busy)).await;

    let deleted = deleted.unwrap();
    assert_eq!(deleted.fulfillers_not_told, [""; 0]);
    assert!(!media.path("Alien (1979)").exists());
    assert_eq!(asked_by_delete, 1);
    asked_again.expect("it was never asked again");
    assert_eq!(busy.asked.load(Ordering::SeqCst), 2);
}
