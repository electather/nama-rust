use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        self,
        SegmentKind::{SEGMENT_KIND_CREDITS, SEGMENT_KIND_INTRO, SEGMENT_KIND_RECAP},
        SegmentResult, SegmentResults, SegmentSearch,
    },
    segment_source::SegmentSource,
};
use tempfile::TempDir;

use super::{Segment, SegmentKind, Segments, segments_of};
use crate::{
    db::{self, Database},
    library::{
        Contents, Libraries,
        fixture::{MatchedMovie, tmdb_movie},
    },
    playback::{Chapter, Probe, Tracks},
    plugins::Registry,
};

/// How long the test Version runs.
const DURATION_MS: i64 = 3_600_000;

/// The ids of the sources asked, in the order asked.
type Asked = Arc<Mutex<Vec<&'static str>>>;

/// The Plugin `id`'s Segment source, answering `answer`, or failing when it's `None`.
#[derive(Clone)]
struct Fake {
    id: &'static str,
    answer: Option<Vec<SegmentResult>>,
    asked: Asked,
}

impl SegmentSource for Fake {
    fn search<'a>(
        &'a self,
        _search: &'a SegmentSearch,
    ) -> BoxFuture<'a, Result<SegmentResults, PluginError>> {
        (self.asked.lock().unwrap_or_else(PoisonError::into_inner)).push(self.id);
        let answer = match &self.answer {
            Some(segments) => Ok(SegmentResults {
                segments: segments.clone(),
                ..SegmentResults::default()
            }),
            None => Err("the service is down".into()),
        };
        Box::pin(std::future::ready(answer))
    }
}

impl Plugin for Fake {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn SegmentSource>()],
            ..Manifest::new(PluginId::from_static(self.id))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn SegmentSource>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

fn result(kind: contract::SegmentKind, start_ms: u64, end_ms: Option<u64>) -> SegmentResult {
    SegmentResult {
        kind: kind.into(),
        start_ms,
        end_ms,
        ..SegmentResult::default()
    }
}

fn segment(kind: SegmentKind, start_ms: i64, end_ms: Option<i64>) -> Segment {
    Segment {
        kind,
        start_ms,
        end_ms,
    }
}

fn chapter(start_ms: i64, title: &str) -> Chapter {
    Chapter {
        start_ms,
        title: title.to_owned(),
    }
}

/// A Matched Movie's Version, probed as [`DURATION_MS`] long with `chapters`, in a Library
/// listing the Segment sources `listed`, with `fakes` enabled.
struct Fixture {
    segments: Segments,
    db: Arc<Database>,
    version: i64,
    asked: Asked,
    plugins: Registry,
    _dirs: (TempDir, TempDir),
}

impl Fixture {
    async fn new(chapters: Vec<Chapter>, listed: &[&str], fakes: Vec<Fake>) -> Self {
        let (data, media) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(348, "Alien")).await;
        probe(&db, alien.version, chapters).await;
        let listed = listed.iter().map(|&id| id.to_owned()).collect();
        (Libraries::new(Arc::clone(&db)).set_segment_sources(alien.library, listed))
            .await
            .unwrap();
        let plugins = Registry::new();
        let asked = Asked::default();
        for fake in fakes {
            let fake = Fake {
                asked: Arc::clone(&asked),
                ..fake
            };
            plugins.enable(Arc::new(fake)).unwrap();
        }
        Self {
            segments: Segments::new(Arc::clone(&db), plugins.extension_points()),
            plugins,
            db,
            version: alien.version,
            asked,
            _dirs: (data, media),
        }
    }

    async fn ask(&self) {
        self.segments.ask(self.version).await.unwrap();
    }

    async fn stored(&self) -> Vec<Segment> {
        segments_of(&self.db, self.version).await.unwrap()
    }

    fn asked(&self) -> Vec<&'static str> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Stores a probe of the Version `version` as [`DURATION_MS`] long with `chapters`, as Probing
/// does each time it probes the file.
async fn probe(db: &Arc<Database>, version: i64, chapters: Vec<Chapter>) {
    let (_, file) = (Contents::new(Arc::clone(db)).version_file(version))
        .await
        .unwrap()
        .unwrap();
    let probe = Probe {
        container: "matroska,webm".to_owned(),
        duration_ms: Some(DURATION_MS),
        bitrate: None,
        tracks: vec![],
        font_attachments: vec![],
        chapters,
    };
    Tracks::new(Arc::clone(db))
        .store(file, probe)
        .await
        .unwrap();
}

fn fake(id: &'static str, answer: Option<Vec<SegmentResult>>) -> Fake {
    Fake {
        id,
        answer,
        asked: Asked::default(),
    }
}

#[tokio::test]
async fn chapters_named_opening_and_ending_give_an_intro_and_credits_without_asking() {
    // "Previously on" names the Recap, so no kind is left for the listed source.
    let chapters = vec![
        chapter(0, "Previously on"),
        chapter(30_000, "Opening"),
        chapter(90_000, ""),
        chapter(3_500_000, "Ending"),
    ];
    let everything = vec![result(SEGMENT_KIND_INTRO, 1, Some(2))];
    let fx = Fixture::new(
        chapters,
        &["source"],
        vec![fake("source", Some(everything))],
    )
    .await;

    fx.ask().await;

    assert_eq!(
        fx.stored().await,
        [
            segment(SegmentKind::Recap, 0, Some(30_000)),
            segment(SegmentKind::Intro, 30_000, Some(90_000)),
            segment(SegmentKind::Credits, 3_500_000, None),
        ]
    );
    assert_eq!(fx.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn kinds_the_chapters_left_come_from_the_first_listed_source_that_answers() {
    let chapters = vec![chapter(0, "OP1"), chapter(60_000, "Part A")];
    let first = vec![result(SEGMENT_KIND_CREDITS, 3_000_000, None)];
    let second = vec![
        result(SEGMENT_KIND_INTRO, 5_000, Some(10_000)),
        result(SEGMENT_KIND_RECAP, 60_000, Some(120_000)),
        result(SEGMENT_KIND_CREDITS, 3_100_000, None),
    ];
    let fakes = vec![
        fake("unlisted", Some(second.clone())),
        fake("disabled", Some(second.clone())),
        fake("first", Some(first)),
        fake("second", Some(second)),
        fake("third", Some(vec![])),
    ];
    let listed = ["gone", "disabled", "first", "second", "third"];
    let fx = Fixture::new(chapters, &listed, fakes).await;
    assert!(fx.plugins.disable_now(&PluginId::from_static("disabled")));

    fx.ask().await;

    assert_eq!(
        fx.stored().await,
        [
            segment(SegmentKind::Intro, 0, Some(60_000)),
            segment(SegmentKind::Recap, 60_000, Some(120_000)),
            segment(SegmentKind::Credits, 3_000_000, None),
        ]
    );
    // Nothing was left to ask `third` for.
    assert_eq!(fx.asked(), ["first", "second"]);
}

#[tokio::test]
async fn segments_out_of_range_are_dropped_or_cut_back_and_a_second_of_a_kind_ignored() {
    let answer = vec![
        result(SEGMENT_KIND_RECAP, 50_000, Some(40_000)),
        result(SEGMENT_KIND_CREDITS, 3_600_001, None),
        result(SEGMENT_KIND_INTRO, 10_000, Some(3_700_000)),
        result(SEGMENT_KIND_INTRO, 20_000, Some(30_000)),
    ];
    let fx = Fixture::new(vec![], &["source"], vec![fake("source", Some(answer))]).await;

    fx.ask().await;

    assert_eq!(
        fx.stored().await,
        [segment(SegmentKind::Intro, 10_000, Some(DURATION_MS))]
    );
}

#[tokio::test]
async fn a_version_a_source_failed_for_is_due_again_within_hours_not_days() {
    let fx = Fixture::new(vec![], &["down"], vec![fake("down", None)]).await;
    assert_eq!(fx.segments.due().await.unwrap(), [fx.version]);

    fx.ask().await;

    assert_eq!(fx.segments.due().await.unwrap(), Vec::<i64>::new());
    let due_ms = sqlx::query_scalar!(
        "SELECT segments_due_ms FROM version_probes WHERE version_id = ?",
        fx.version
    )
    .fetch_one(fx.db.reader())
    .await
    .unwrap()
    .unwrap();
    let wait_ms = due_ms - db::now_ms();
    assert!((0..=3_600_000).contains(&wait_ms), "{wait_ms}");
}

#[tokio::test]
async fn a_versions_segments_go_when_it_is_probed_again_and_it_is_asked_again() {
    let fx = Fixture::new(
        vec![chapter(0, "Opening"), chapter(60_000, "")],
        &[],
        vec![],
    )
    .await;
    fx.ask().await;
    assert_eq!(fx.segments.due().await.unwrap(), Vec::<i64>::new());

    probe(&fx.db, fx.version, vec![]).await;

    assert_eq!(fx.stored().await, []);
    assert_eq!(fx.segments.due().await.unwrap(), [fx.version]);
}
