use std::{fs, io::Write as _, path::PathBuf, sync::Arc, time::Duration};

use futures_util::StreamExt;
use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::{Lagged, subscribe_response};
use tempfile::TempDir;

use super::{Planned, Scanner, Summary, ToDelete};
use crate::{
    db::Database,
    events::{EventBus, Filter},
    ffmpeg::Ffmpeg,
    images::ImageStore,
    library::{
        Artwork, Change, Contents, CustomImages, Decided, File, Item, Libraries, LibraryError,
        LibraryKind, LocalImages, Order, TitleTarget,
        fixture::{decide, tmdb_movie},
    },
    log_capture::CapturedLog,
    playback::{Probing, RunningPlans, SubtitleCache},
    plugin_settings::PluginSettings,
    plugins::{ExtensionPoints, Registry},
    requests::Requests,
    server_settings::ServerUrl,
};

/// How many events a [`Fixture`]'s subscribers may fall behind.
const BUFFERED: usize = 8;

/// A database, a Library of `kind` backed by one scratch folder, and a scanner publishing on
/// a bus whose subscribers lag once [`BUFFERED`] events behind.
pub(super) struct Fixture {
    pub(super) scanner: Scanner,
    /// What the scanner tells a scan finished, if anything probes.
    pub(super) probing: Option<Probing>,
    pub(super) db: Arc<Database>,
    pub(super) contents: Contents,
    pub(super) events: EventBus,
    pub(super) library: i64,
    pub(super) root: PathBuf,
    _media: TempDir,
    _data: TempDir,
}

impl Fixture {
    /// With nothing probing.
    pub(super) async fn new(kind: LibraryKind) -> Self {
        Self::with_probing(kind, None, Registry::new()).await
    }

    /// With nothing probing, and `plugins` as the Plugins the scanner asks.
    pub(super) async fn with_plugins(kind: LibraryKind, plugins: Registry) -> Self {
        Self::with_probing(kind, None, plugins).await
    }

    /// Probing with `ffmpeg`.
    #[cfg(unix)]
    pub(super) async fn with_ffmpeg(kind: LibraryKind, ffmpeg: Ffmpeg) -> Self {
        Self::with_probing(kind, Some(ffmpeg), Registry::new()).await
    }

    async fn with_probing(kind: LibraryKind, ffmpeg: Option<Ffmpeg>, plugins: Registry) -> Self {
        let data = tempfile::tempdir().unwrap();
        let media = tempfile::tempdir().unwrap();
        let root = media.path().canonicalize().unwrap().join("library");
        fs::create_dir(&root).unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let library = Libraries::new(Arc::clone(&db))
            .create("Media", kind, vec![root.to_str().unwrap().to_owned()])
            .await
            .unwrap();
        let events = EventBus::with_capacity(
            Arc::clone(&db),
            ExtensionPoints::default(),
            ServerUrl::default(),
            BUFFERED,
        );
        let images = ImageStore::new(data.path(), Arc::clone(&db));
        let probing = ffmpeg.map(|ffmpeg| {
            let subtitles = SubtitleCache::new(data.path());
            Probing::new(Arc::clone(&db), Arc::new(ffmpeg), subtitles, images.clone())
        });
        Self {
            scanner: Scanner::new(
                Arc::clone(&db),
                PluginSettings::for_tests(Arc::clone(&db), plugins),
                Artwork::new(images.clone(), ExtensionPoints::default()),
                CustomImages::new(Arc::clone(&db), images.clone()),
                LocalImages::new(Arc::clone(&db), images),
                events.clone(),
                probing.clone(),
                RunningPlans::for_tests(&db, events.clone(), data.path()),
                Requests::new(Arc::clone(&db), events.clone(), ExtensionPoints::default()),
            ),
            probing,
            contents: Contents::new(Arc::clone(&db)),
            db,
            events,
            library: library.id,
            root,
            _media: media,
            _data: data,
        }
    }

    pub(super) fn write(&self, relative: &str, size: usize) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0_u8; size]).unwrap();
    }

    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub(super) async fn scan(&self) -> Summary {
        self.scanner.scan(self.library).await.unwrap()
    }

    /// Matches the scanned file at `relative` to the Movie `tmdb`, called `title`, and names it
    /// to delete.
    pub(super) async fn movie(&self, relative: &str, tmdb: i64, title: &str) -> ToDelete {
        let path = self.path(relative);
        let matched = vec![Decided::Matched(Box::new(tmdb_movie(tmdb, title)))];
        decide(
            &self.contents,
            self.library,
            path.to_str().unwrap(),
            matched,
        )
        .await;
        ToDelete::Title {
            library: self.library,
            title: TitleTarget::Movie(ExternalId::tmdb(tmdb)),
        }
    }

    /// The Library's folders its last scan held because they read empty.
    async fn empty_folders(&self) -> Vec<String> {
        let libraries = &self.scanner.inner.libraries;
        libraries.get(self.library).await.unwrap().empty_folders
    }

    /// Makes every later write fail, as a database that can't be written to does.
    async fn stop_writes(&self) {
        self.db
            .write(|tx| Box::pin(sqlx::query("PRAGMA query_only = ON").execute(&mut **tx)))
            .await
            .unwrap();
    }

    /// The stored files, as `/`-separated paths relative to the Library folder, whatever
    /// separator the platform uses.
    pub(super) async fn files(&self) -> Vec<(String, Vec<Item>)> {
        self.contents
            .files(self.library)
            .await
            .unwrap()
            .into_iter()
            .map(|File { path, items, .. }| {
                let path = PathBuf::from(path);
                let relative = path.strip_prefix(&self.root).unwrap();
                let parts: Vec<&str> = relative
                    .components()
                    .map(|part| part.as_os_str().to_str().unwrap())
                    .collect();
                let items = items.into_iter().map(|stored| stored.item).collect();
                (parts.join("/"), items)
            })
            .collect()
    }
}

fn movie(title: &str, year: i64, hint: Option<ExternalId>) -> Vec<Item> {
    vec![Item::Movie {
        title: title.to_owned(),
        year: Some(year),
        hint,
    }]
}

pub(super) fn summary(added: u64, updated: u64, removed: u64, unchanged: u64) -> Summary {
    Summary {
        added,
        updated,
        removed,
        unchanged,
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "left out a change the scan never counted as updated")]
fn leaving_out_more_than_was_updated_is_caught_in_debug_builds() {
    let mut counted = summary(0, 0, 1, 0);
    let removal = Planned {
        change: Change::Remove(7),
        unread: None,
        added: false,
    };

    counted.leave_out(&[removal]);
}

#[tokio::test]
async fn scanning_a_movies_library_stores_its_video_files_and_what_they_hold() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Blade Runner (1982)/Blade Runner (1982).mkv", 10);
    media.write("Alien (1979)/Alien (1979).MP4", 10);
    // Not video, an extra, hidden, and a name that doesn't parse.
    media.write("Alien (1979)/Alien (1979).srt", 10);
    media.write("Alien (1979)/Alien (1979).nfo", 10);
    media.write("Alien (1979)/Featurettes/Making Alien.mkv", 10);
    media.write("Alien (1979)/Alien (1979)-trailer.mkv", 10);
    media.write(".trash/Heat (1995).mkv", 10);
    media.write("Alien (1979)/._Alien (1979).mkv", 10);
    media.write("home video.mkv", 10);

    assert_eq!(media.scan().await, summary(3, 0, 0, 0));
    assert_eq!(
        media.files().await,
        [
            (
                "Alien (1979)/Alien (1979).MP4".to_owned(),
                movie("Alien", 1979, None)
            ),
            (
                "Blade Runner (1982)/Blade Runner (1982).mkv".to_owned(),
                movie("Blade Runner", 1982, None)
            ),
            // Stored, but Unmatched: there's nothing to Match it by.
            ("home video.mkv".to_owned(), vec![]),
        ]
    );
}

#[tokio::test]
async fn scanning_a_shows_library_stores_an_item_per_episode() {
    let media = Fixture::new(LibraryKind::Shows).await;
    media.write("Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv", 10);
    media.write("Dark (2017)/Season 00/Dark (2017) - S00E01.mkv", 10);

    assert_eq!(media.scan().await, summary(2, 0, 0, 0));
    let episode = |season, episode| Item::Episode {
        show: "Dark".to_owned(),
        year: Some(2017),
        hint: None,
        season,
        episode,
    };
    assert_eq!(
        media.files().await,
        [
            (
                "Dark (2017)/Season 00/Dark (2017) - S00E01.mkv".to_owned(),
                vec![episode(0, 1)]
            ),
            (
                "Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv".to_owned(),
                vec![episode(1, 1), episode(1, 2)]
            ),
        ]
    );
}

#[tokio::test]
async fn writes_failing_once_the_files_are_gone_are_logged_and_the_delete_still_reports_them() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;
    let alien = media
        .movie("Alien (1979)/Alien (1979).mkv", 348, "Alien")
        .await;
    media.stop_writes().await;
    // The test's runtime has one thread, so the delete's tasks log to it too.
    let log = CapturedLog::start();

    let deleted = (media.scanner)
        .delete_media(alien, Vec::new(), "ada".to_owned())
        .await;

    let log = log.finish();
    let deleted = deleted.unwrap();
    let files: Vec<_> = deleted.files.iter().map(|file| &file.path).collect();
    assert_eq!(files, [&media.path("Alien (1979)/Alien (1979).mkv")]);
    assert_eq!(deleted.bytes_freed(), 10);
    assert!(!media.path("Alien (1979)").exists());
    let logged = |step: &str| {
        log.lines()
            .any(|line| line.contains("ERROR") && line.contains(step))
    };
    assert!(logged("couldn't remove the deleted Versions"), "{log}");
    assert!(logged("couldn't cancel the Requests"), "{log}");
}

#[tokio::test]
async fn a_rescan_skips_unchanged_files_and_picks_up_added_changed_renamed_and_deleted_ones() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Blade Runner (1982)/Blade Runner (1982).mkv", 10);
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.write("Heat (1995)/Heat (1995).mkv", 10);
    media.write("home video.mkv", 10);
    assert_eq!(media.scan().await, summary(4, 0, 0, 0));
    assert_eq!(media.scan().await, summary(0, 0, 0, 4), "nothing changed");

    media.write("Up (2009)/Up (2009).mkv", 10);
    media.write("Alien (1979)/Alien (1979).mkv", 20);
    // Renamed, with an ID hint.
    fs::rename(
        media.path("Blade Runner (1982)"),
        media.path("Blade Runner (1982) {tmdb-78}"),
    )
    .unwrap();
    fs::remove_file(media.path("home video.mkv")).unwrap();

    assert_eq!(media.scan().await, summary(2, 1, 2, 1));
    assert_eq!(
        media.files().await,
        [
            (
                "Alien (1979)/Alien (1979).mkv".to_owned(),
                movie("Alien", 1979, None)
            ),
            (
                "Blade Runner (1982) {tmdb-78}/Blade Runner (1982).mkv".to_owned(),
                movie("Blade Runner", 1982, Some(ExternalId::tmdb(78)))
            ),
            (
                "Heat (1995)/Heat (1995).mkv".to_owned(),
                movie("Heat", 1995, None)
            ),
            (
                "Up (2009)/Up (2009).mkv".to_owned(),
                movie("Up", 2009, None)
            ),
        ]
    );
}

#[tokio::test]
async fn a_newer_name_parser_parses_unchanged_files_again_once() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.write("home video.mkv", 10);
    media.scan().await;

    media.contents.read_by_older_parser(media.library).await;

    assert_eq!(media.scan().await, summary(0, 2, 0, 0));
    assert_eq!(media.scan().await, summary(0, 0, 0, 2));
    assert_eq!(media.files().await[0].1, movie("Alien", 1979, None));
}

#[tokio::test]
async fn files_in_a_folder_that_cant_be_read_are_kept() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    assert_eq!(media.scan().await, summary(1, 0, 0, 0));

    // The Library folder is gone, as when a drive isn't mounted.
    let parked = media.root.with_file_name("parked");
    fs::rename(&media.root, &parked).unwrap();
    assert_eq!(media.scan().await, summary(0, 0, 0, 1));
    assert_eq!(
        media.files().await.len(),
        1,
        "an unreadable folder emptied the Library"
    );
    assert_eq!(
        media.empty_folders().await,
        Vec::<String>::new(),
        "an unreadable folder looked empty, which removing its missing items can't clear"
    );
    fs::rename(&parked, &media.root).unwrap();

    assert_eq!(media.scan().await, summary(0, 0, 0, 1));
}

#[tokio::test]
async fn a_folder_that_reads_empty_keeps_its_items_and_looks_empty_until_files_are_back() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;

    // An unmounted share's mountpoint is an empty folder.
    fs::rename(&media.root, media.root.with_file_name("parked")).unwrap();
    fs::create_dir(&media.root).unwrap();
    assert_eq!(media.scan().await, summary(0, 0, 0, 1));
    assert_eq!(
        media.files().await.len(),
        1,
        "an empty folder emptied the Library"
    );
    assert_eq!(media.empty_folders().await, [media.root.to_str().unwrap()]);

    media.write("Up (2009)/Up (2009).mkv", 10);
    assert_eq!(media.scan().await, summary(1, 0, 1, 0));
    assert_eq!(media.empty_folders().await, Vec::<String>::new());
}

#[tokio::test]
async fn removing_missing_items_empties_a_folder_that_looks_empty_once() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;
    fs::remove_dir_all(media.path("Alien (1979)")).unwrap();
    media.scan().await;
    let folder = media.root.to_str().unwrap().to_owned();

    (media.scanner.remove_missing(media.library, folder))
        .await
        .unwrap();
    media.scanner.idle().await;

    assert_eq!(media.files().await.len(), 0);
    assert_eq!(media.empty_folders().await, Vec::<String>::new());
    media.write("Up (2009)/Up (2009).mkv", 10);
    media.scan().await;
    fs::remove_dir_all(media.path("Up (2009)")).unwrap();
    assert_eq!(
        media.scan().await,
        summary(0, 0, 0, 1),
        "the removal held only once"
    );
}

#[tokio::test]
async fn removing_missing_items_from_a_folder_not_yet_held_keeps_them() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;
    fs::remove_dir_all(media.path("Alien (1979)")).unwrap();
    let folder = media.root.to_str().unwrap().to_owned();

    (media.scanner.remove_missing(media.library, folder.clone()))
        .await
        .unwrap();
    media.scanner.idle().await;

    assert_eq!(media.files().await.len(), 1);
    assert_eq!(media.empty_folders().await, [folder]);
}

#[tokio::test]
async fn a_folder_left_with_few_files_or_none_ever_doesnt_look_empty() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.scan().await;
    assert_eq!(
        media.empty_folders().await,
        Vec::<String>::new(),
        "a new, empty folder"
    );

    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.write("Up (2009)/Up (2009).mkv", 10);
    media.scan().await;
    fs::remove_dir_all(media.path("Up (2009)")).unwrap();
    assert_eq!(media.scan().await, summary(0, 0, 1, 1));
    assert_eq!(media.empty_folders().await, Vec::<String>::new());
}

#[tokio::test]
async fn a_scan_started_in_the_background_stores_the_files() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);

    media.scanner.start(media.library).await.unwrap();
    // Asking again while the first is queued or running changes nothing it finds.
    media.scanner.start(media.library).await.unwrap();
    media.scanner.idle().await;

    assert_eq!(
        media.files().await,
        [(
            "Alien (1979)/Alien (1979).mkv".to_owned(),
            movie("Alien", 1979, None)
        )]
    );
}

#[tokio::test]
async fn every_library_is_scanned_when_asked_to_scan_all() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);

    media.scanner.start_all().await;
    media.scanner.idle().await;

    assert_eq!(media.files().await.len(), 1);
}

#[tokio::test]
async fn starting_a_scan_of_a_library_that_doesnt_exist_fails() {
    let media = Fixture::new(LibraryKind::Movies).await;
    let result = media.scanner.start(media.library + 1).await;
    assert!(
        matches!(result, Err(LibraryError::NotFound(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn shutting_down_stops_scans_and_starts_no_more() {
    let media = Fixture::new(LibraryKind::Movies).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scanner.start(media.library).await.unwrap();
    media.scanner.shutdown().await;

    let before = media.files().await.len();
    media.scanner.start(media.library).await.unwrap();
    media.scanner.idle().await;
    assert_eq!(
        media.files().await.len(),
        before,
        "a scan ran after shutdown"
    );
}

#[tokio::test]
async fn a_subscriber_that_falls_behind_lags_while_the_scan_finishes_on_time() {
    let media = Fixture::new(LibraryKind::Movies).await;
    for n in 0..20 {
        media.write(&format!("Movie {n} (2000)/Movie {n} (2000).mkv"), 10);
    }
    // Never read while the scan runs.
    let mut slow = Box::pin(media.events.subscribe_as_admin(Filter::default()));

    let scanned =
        tokio::time::timeout(Duration::from_secs(5), media.scanner.scan(media.library)).await;
    assert!(matches!(scanned, Ok(Ok(_))), "{scanned:?}");

    // The scan published 25 events: its start, one batch's progress, 20 Items, its finish, and
    // Matching's and Probing's finish. Only the last 8 were kept.
    let lagged = Lagged {
        dropped: 17,
        ..Default::default()
    };
    assert_eq!(
        slow.next().await.and_then(|response| response.message),
        Some(subscribe_response::Message::from(lagged))
    );
}

/// An ffprobe in `bin` that adds a line to `bin/runs` each time it runs, then runs `then`, a
/// shell script.
#[cfg(unix)]
pub(super) fn fake_ffprobe(bin: &std::path::Path, then: &str) -> Ffmpeg {
    let runs = bin.join("runs");
    let script = format!("#!/bin/sh\necho run >> '{}'\n{then}\n", runs.display());
    Ffmpeg {
        ffprobe: Ok(crate::ffmpeg::fake::script(bin, "ffprobe", &script)),
        ..Ffmpeg::default()
    }
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_scan_probes_its_new_files_in_the_background() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffprobe(bin.path(), "exit 1");
    let media = Fixture::with_ffmpeg(LibraryKind::Movies, ffmpeg).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);

    media.scan().await;
    media.probing.as_ref().unwrap().idle().await;

    assert!(bin.path().join("runs").exists(), "nothing was probed");
}

// The fake ffprobe is a shell script.
#[cfg(unix)]
#[tokio::test]
async fn a_scan_finishes_without_waiting_for_probing() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffprobe(bin.path(), "exec sleep 60");
    let media = Fixture::with_ffmpeg(LibraryKind::Movies, ffmpeg).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);

    let scanned = tokio::time::timeout(Duration::from_secs(5), media.scan()).await;

    assert_eq!(scanned, Ok(summary(1, 0, 0, 0)));
    media.probing.as_ref().unwrap().shutdown().await;
}

/// How long a watched Library's folders stay quiet in these tests before it's scanned.
const QUIET: Duration = Duration::from_secs(1);

/// The periodic scan's interval, so it never comes within a test.
const LONGER_THAN_A_TEST: Duration = Duration::from_secs(3600);

/// How many 50 ms polls the watch tests wait, 60 s in all. macOS's fseventsd falls behind
/// when builds run beside the tests, and starting a watch has then taken over 10 s.
const POLLS: usize = 1200;

/// Starts watching, and waits for the Library's folder to be watched.
async fn watch(media: &Fixture) {
    media.scanner.watch(QUIET, LONGER_THAN_A_TEST);
    for _ in 0..POLLS {
        if !media.scanner.watched().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the Library's folder was never watched");
}

/// Waits for the stored files to be `expected`.
async fn wait_for_files(media: &Fixture, expected: &[&str]) {
    let names = async || -> Vec<String> {
        media
            .files()
            .await
            .into_iter()
            .map(|(path, _)| path)
            .collect()
    };
    for _ in 0..POLLS {
        if names().await == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(names().await, expected);
}

#[tokio::test]
async fn a_watched_library_follows_files_added_renamed_and_deleted_without_being_asked() {
    let media = Fixture::new(LibraryKind::Movies).await;
    watch(&media).await;

    media.write("Alien (1979).mkv", 10);
    wait_for_files(&media, &["Alien (1979).mkv"]).await;

    fs::rename(
        media.path("Alien (1979).mkv"),
        media.path("Aliens (1986).mkv"),
    )
    .unwrap();
    wait_for_files(&media, &["Aliens (1986).mkv"]).await;

    // Any file keeps the folder from reading empty, which would keep the deleted one.
    media.write("notes.txt", 1);
    fs::remove_file(media.path("Aliens (1986).mkv")).unwrap();
    wait_for_files(&media, &[]).await;
    media.scanner.shutdown().await;
}

#[tokio::test]
async fn a_file_written_in_chunks_is_scanned_once() {
    let media = Fixture::new(LibraryKind::Movies).await;
    watch(&media).await;

    for _ in 0..5 {
        // Closed after each chunk: macOS reports a file's writes only once it's closed.
        let mut file = fs::File::options()
            .create(true)
            .append(true)
            .open(media.path("Alien (1979).mkv"))
            .unwrap();
        file.write_all(&[0; 1024]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        tokio::time::sleep(QUIET / 5).await;
    }
    wait_for_files(&media, &["Alien (1979).mkv"]).await;
    tokio::time::sleep(QUIET * 2).await;

    let scans = super::lock(&media.scanner.inner.started)[&media.library];
    assert_eq!(scans, 1);
    media.scanner.shutdown().await;
}

#[tokio::test]
async fn watches_follow_libraries_created_updated_and_deleted() {
    // Built-in TMDB, as a Server has, keeps the titles of a new Library by default.
    let media = Fixture::with_plugins(LibraryKind::Movies, Registry::with_builtins()).await;
    let watched = || media.scanner.watched();
    let more = tempfile::tempdir().unwrap();
    let (first, second) = (more.path().join("first"), more.path().join("second"));
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let first = first.canonicalize().unwrap();
    let second = second.canonicalize().unwrap();
    watch(&media).await;

    let created = media
        .scanner
        .create_library(
            "More".to_owned(),
            LibraryKind::Shows,
            vec![first.to_str().unwrap().to_owned()],
            Order::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        watched(),
        [(media.root.clone(), media.library), (first, created.id)]
    );

    media
        .scanner
        .update_library(
            created.id,
            "More".to_owned(),
            vec![second.to_str().unwrap().to_owned()],
            Order::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        watched(),
        [(media.root.clone(), media.library), (second, created.id)]
    );

    media.scanner.delete_library(created.id).await.unwrap();
    assert_eq!(watched(), [(media.root.clone(), media.library)]);
    media.scanner.shutdown().await;
}

#[tokio::test]
async fn deleting_a_library_inside_another_keeps_the_outer_one_watched() {
    // Built-in TMDB, as a Server has, keeps the titles of a new Library by default.
    let media = Fixture::with_plugins(LibraryKind::Movies, Registry::with_builtins()).await;
    let beside = media.root.with_file_name("beside");
    fs::create_dir(&beside).unwrap();
    fs::create_dir(media.path("inner")).unwrap();
    let inner = media.path("inner").to_str().unwrap().to_owned();
    let created = media
        .scanner
        .create_library(
            "Inner".to_owned(),
            LibraryKind::Shows,
            vec![beside.to_str().unwrap().to_owned()],
            Order::default(),
        )
        .await
        .unwrap();
    // Only a Library created before overlaps were refused (#812) can be inside another.
    media
        .db
        .write(move |tx| {
            Box::pin(
                sqlx::query("UPDATE library_folders SET path = ? WHERE library_id = ?")
                    .bind(inner)
                    .bind(created.id)
                    .execute(&mut **tx),
            )
        })
        .await
        .unwrap();
    watch(&media).await;
    assert_eq!(
        media.scanner.watched().len(),
        2,
        "both Libraries are watched"
    );

    media.scanner.delete_library(created.id).await.unwrap();
    media.write("inner/Alien (1979).mkv", 10);

    wait_for_files(&media, &["inner/Alien (1979).mkv"]).await;
    media.scanner.shutdown().await;
}
