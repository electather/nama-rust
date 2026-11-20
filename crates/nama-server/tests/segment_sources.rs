//! Segment sources over the API (ADR 0075): a Library lists the Segment sources it asks, in an
//! order the Admin sets, and the Server asks them in the background after probing for the kinds of
//! Segment a Version's Chapter names didn't give, which its Playback Plans then send.

#[path = "support/browsing.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod browsing;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::sync::{Arc, Mutex, PoisonError};

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{SegmentResult, SegmentResults, SegmentSearch},
    segment_source::SegmentSource,
};
use nama_proto::nama::v1::{
    CreateLibraryRequest, ExtensionPoint, LibraryKind, LibraryServiceClient, ListPluginsRequest,
    PluginServiceClient, RefusalReason, SegmentSourceOrder, UpdateLibraryRequest,
    UpdateLibraryResponse,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::{Builder, Library, TestServer};

/// The id of [`Scripted`]'s Plugin.
const SOURCE: &str = "scripted";

/// A Built-in Segment source answering what it's scripted to, and noting the duration of each
/// Version it's asked about.
#[derive(Clone, Default)]
struct Scripted {
    answer: Arc<Mutex<Vec<SegmentResult>>>,
    asked: Arc<Mutex<Vec<u64>>>,
}

impl Scripted {
    fn asked(&self) -> Vec<u64> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SegmentSource for Scripted {
    fn search<'a>(
        &'a self,
        search: &'a SegmentSearch,
    ) -> BoxFuture<'a, Result<SegmentResults, PluginError>> {
        (self.asked.lock().unwrap_or_else(PoisonError::into_inner)).push(search.duration_ms);
        let segments = self
            .answer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Box::pin(std::future::ready(Ok(SegmentResults {
            segments,
            ..SegmentResults::default()
        })))
    }
}

impl Plugin for Scripted {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn SegmentSource>()],
            ..Manifest::new(PluginId::from_static(SOURCE))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn SegmentSource>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

/// `builder` with `source` enabled as a Built-in Plugin.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn with_source(builder: Builder, source: &Scripted) -> Builder {
    let plugins = Registry::new();
    let made = source.clone();
    plugins.describe_builtin(move || Arc::new(made.clone()));
    plugins.enable(Arc::new(source.clone())).unwrap();
    builder.plugins(plugins)
}

fn libraries(server: &TestServer) -> LibraryServiceClient<HttpClient> {
    LibraryServiceClient::new(HttpClient::plaintext(), server.client_config())
}

fn ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|&id| id.to_owned()).collect()
}

/// Updates `library` as the Admin, keeping its name and folder, with `order` as its
/// `segment_source_order`.
async fn update_segment_sources(
    server: &TestServer,
    library: &Library,
    order: &[&str],
) -> Result<UpdateLibraryResponse, ConnectError> {
    libraries(server)
        .update_library(UpdateLibraryRequest {
            id: library.id,
            name: library.folder().to_string_lossy().into_owned(),
            folders: vec![library.folder().to_string_lossy().into_owned()],
            segment_source_order: SegmentSourceOrder {
                ids: ids(order),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

#[tokio::test]
async fn a_library_round_trips_its_segment_sources_on_create_and_update() {
    let source = Scripted::default();
    let server = with_source(TestServer::builder(), &source).start().await;
    let folder = tempfile::tempdir().unwrap();

    let created = libraries(&server)
        .create_library(CreateLibraryRequest {
            name: "Shows".to_owned(),
            kind: LibraryKind::LIBRARY_KIND_SHOWS.into(),
            folders: vec![folder.path().to_string_lossy().into_owned()],
            segment_source_ids: ids(&[SOURCE]),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .library
        .into_option()
        .unwrap();
    assert_eq!(created.segment_source_ids, [SOURCE]);

    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    let updated = update_segment_sources(&server, &library, &[SOURCE])
        .await
        .unwrap();
    assert_eq!(
        updated.library.into_option().unwrap().segment_source_ids,
        [SOURCE]
    );
    let cleared = update_segment_sources(&server, &library, &[])
        .await
        .unwrap();
    assert_eq!(
        cleared.library.into_option().unwrap().segment_source_ids,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_segment_source_no_plugin_serves_or_named_twice_is_refused_naming_the_field() {
    let source = Scripted::default();
    let server = with_source(TestServer::builder(), &source).start().await;
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    // `local-auth` is a Plugin, but no Segment source; `nope` is no Plugin at all.
    for (order, refused_id) in [
        (&[SOURCE, "nope"][..], "nope"),
        (&[SOURCE, "local-auth"], "local-auth"),
        (&[SOURCE, SOURCE], SOURCE),
    ] {
        let folder = tempfile::tempdir().unwrap();
        let created = libraries(&server)
            .create_library(CreateLibraryRequest {
                name: "Shows".to_owned(),
                kind: LibraryKind::LIBRARY_KIND_SHOWS.into(),
                folders: vec![folder.path().to_string_lossy().into_owned()],
                segment_source_ids: ids(order),
                ..Default::default()
            })
            .await;
        let updated = update_segment_sources(&server, &library, order).await;

        for (refused, field) in [
            (created.map(drop), "segment_source_ids"),
            (updated.map(drop), "segment_source_order"),
        ] {
            let refusal = assert_refused(
                refused,
                ErrorCode::InvalidArgument,
                RefusalReason::REFUSAL_REASON_SEGMENT_SOURCES_INVALID,
            );
            assert_eq!(refusal.field, field, "{order:?}");
            assert_eq!(
                refusal.params.get("pluginId").map(String::as_str),
                Some(refused_id),
                "{order:?}"
            );
        }
    }
}

#[tokio::test]
async fn list_plugins_says_which_plugins_are_segment_sources() {
    let source = Scripted::default();
    let server = with_source(TestServer::builder(), &source).start().await;

    let listed = PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned();

    let sources: Vec<&str> = (listed.summaries.iter())
        .filter(|plugin| {
            (plugin.extension_points.iter())
                .any(|point| *point == ExtensionPoint::EXTENSION_POINT_SEGMENT_SOURCE)
        })
        .map(|plugin| plugin.id.as_str())
        .collect();
    assert_eq!(sources, [SOURCE, "theintrodb"]);
}

/// Playback Plans sending what was found, with a fake ffprobe, a shell script.
#[cfg(unix)]
mod plans {
    use std::{path::Path, time::Duration};

    use nama_plugin_api::contract::SegmentKind::{
        self as Theirs, SEGMENT_KIND_CREDITS, SEGMENT_KIND_RECAP,
    };
    use nama_proto::nama::v1::{PlanPlaybackRequest, PlaybackServiceClient, Segment, SegmentKind};
    use nama_server::ffmpeg::{Ffmpeg, fake::script};

    use super::*;
    use crate::{
        browsing::{ALIEN_1080P, ALIEN_2160P, Fixture},
        server::{WAIT, tmdb::ALIEN},
    };

    /// TMDB Matches it to Alien, as [`ALIEN_1080P`] and [`ALIEN_2160P`].
    const ALIEN_720P: &str = "Alien (1979)/Alien (1979) - 720p.mkv";

    impl Scripted {
        fn answer(&self, segments: Vec<SegmentResult>) {
            *self.answer.lock().unwrap_or_else(PoisonError::into_inner) = segments;
        }
    }

    /// An ffprobe, a shell script in `bin`, that reads Alien's 720p file as an hour long with an
    /// "Opening" Chapter and an untitled one a minute in, its 1080p file as 3700 s long and any other
    /// as 3800 s long, both without Chapters, so [`Scripted`] notes which it's asked about.
    fn ffprobe(bin: &Path) -> Ffmpeg {
        let ffprobe = r#"#!/bin/sh
case "$*" in
  *720p*) duration=3600; chapters='[{"start_time": "0.000000", "tags": {"title": "Opening"}},
                                    {"start_time": "60.000000"}]' ;;
  *1080p*) duration=3700; chapters='[]' ;;
  *) duration=3800; chapters='[]' ;;
esac
cat <<EOF
{"streams": [{"codec_name": "h264", "codec_type": "video"},
             {"codec_name": "aac", "codec_type": "audio", "disposition": {"default": 1}}],
 "format": {"format_name": "matroska,webm", "duration": "$duration.000000"},
 "chapters": $chapters}
EOF
"#;
        Ffmpeg {
            ffprobe: Ok(script(bin, "ffprobe", ffprobe)),
            ..Ffmpeg::default()
        }
    }

    fn result(kind: Theirs, start_ms: u64, end_ms: Option<u64>) -> SegmentResult {
        SegmentResult {
            kind: kind.into(),
            start_ms,
            end_ms,
            ..SegmentResult::default()
        }
    }

    fn segment(kind: SegmentKind, start_ms: u64, end_ms: Option<u64>) -> Segment {
        Segment {
            kind: kind.into(),
            start_ms,
            end_ms,
            ..Default::default()
        }
    }

    /// A Movies Library on a Server `builder` starts with `source` enabled and the fake [`ffprobe`]
    /// in `bin`, listing `source` as its Segment source.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn listing_the_source(builder: Builder, source: &Scripted, bin: &Path) -> Fixture {
        let builder = with_source(builder.ffmpeg(ffprobe(bin)), source);
        let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
        update_segment_sources(&fx.server, &fx.media, &[SOURCE])
            .await
            .unwrap();
        fx
    }

    /// Writes `file`, scans the Library and waits until the Version it adds has Segments, which come
    /// in the background after probing, and returns its id and theirs.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn add_and_wait_for_segments(fx: &Fixture, file: &str) -> (i64, Vec<Segment>) {
        let version_ids = async || {
            (fx.get_movie(fx.library, ALIEN).await).map_or_else(
                |_none_yet| Vec::new(),
                |movie| movie.versions.iter().map(|version| version.id).collect(),
            )
        };
        let before: Vec<i64> = version_ids().await;
        fx.write(file, b"video");
        fx.scan_and_match().await;
        let after = version_ids().await;
        let version = *after.iter().find(|id| !before.contains(id)).unwrap();
        let found = async {
            let mut segments = segments_planned(fx, version).await;
            while segments.is_empty() {
                tokio::time::sleep(Duration::from_millis(50)).await;
                segments = segments_planned(fx, version).await;
            }
            segments
        };
        (version, tokio::time::timeout(WAIT, found).await.unwrap())
    }

    /// The Segments a Playback Plan of `version` sends.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn segments_planned(fx: &Fixture, version: i64) -> Vec<Segment> {
        PlaybackServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
            .plan_playback(PlanPlaybackRequest {
                version_id: version,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .plan
            .into_option()
            .unwrap()
            .segments
    }

    #[tokio::test]
    async fn a_plan_sends_the_segments_from_chapter_names_and_the_librarys_source() {
        let (bin, source) = (tempfile::tempdir().unwrap(), Scripted::default());
        source.answer(vec![result(SEGMENT_KIND_CREDITS, 3_000_000, None)]);
        let fx = listing_the_source(TestServer::builder(), &source, bin.path()).await;

        let (_, segments) = add_and_wait_for_segments(&fx, ALIEN_720P).await;

        assert_eq!(
            segments,
            [
                segment(SegmentKind::SEGMENT_KIND_INTRO, 0, Some(60_000)),
                segment(SegmentKind::SEGMENT_KIND_CREDITS, 3_000_000, None),
            ]
        );
    }

    #[tokio::test]
    async fn a_version_missing_a_kind_is_asked_again_only_after_30_days_and_only_for_it() {
        let (bin, source) = (tempfile::tempdir().unwrap(), Scripted::default());
        source.answer(vec![result(SEGMENT_KIND_CREDITS, 3_000_000, None)]);
        let fx = listing_the_source(TestServer::builder().moves_clock(), &source, bin.path()).await;
        let (missing_recap, _) = add_and_wait_for_segments(&fx, ALIEN_720P).await;
        source.answer(vec![
            result(SEGMENT_KIND_RECAP, 60_000, Some(120_000)),
            result(SEGMENT_KIND_CREDITS, 3_100_000, None),
        ]);

        // Each Version added after it is asked about after it, once the pass has looked at it.
        fx.server.advance_clock(Duration::from_hours(29 * 24));
        add_and_wait_for_segments(&fx, ALIEN_1080P).await;
        assert_eq!(source.asked(), [3_600_000, 3_700_000]);

        fx.server.advance_clock(Duration::from_hours(2 * 24));
        add_and_wait_for_segments(&fx, ALIEN_2160P).await;
        assert_eq!(source.asked(), [3_600_000, 3_700_000, 3_600_000, 3_800_000]);
        assert_eq!(
            segments_planned(&fx, missing_recap).await,
            [
                segment(SegmentKind::SEGMENT_KIND_INTRO, 0, Some(60_000)),
                segment(SegmentKind::SEGMENT_KIND_RECAP, 60_000, Some(120_000)),
                segment(SegmentKind::SEGMENT_KIND_CREDITS, 3_000_000, None),
            ]
        );
    }
}
