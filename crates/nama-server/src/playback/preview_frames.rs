//! Preview frames: a still of each Version every [`INTERVAL_S`] seconds, packed into JPEG sheets,
//! for the seek bar to show the frame at a time (ADR 0076).
//!
//! They're made in the background after a probe pass, one Version at a time and only while
//! nothing plays, for the Libraries that make them. Which Versions need them is worked out from
//! what's stored ([`PreviewFrames::needed`]), and [`PreviewFrames::make`] makes one Version's, so a
//! jobs system can take either over. A Version's sheets are kept in the blob store under
//! [`PREVIEW_FRAMES_DIR`], keyed by the file they were made of, and its row in `preview_frames`
//! records what was made, or that making them failed, of the file at its size and modified time:
//! a failure isn't tried again until the file changes. [`Probing::clean_leftovers`] deletes a gone
//! Version's sheets, and its row goes with it.
//!
//! Clients fetch sheet `n` at `/media/versions/<id>/preview-frames/<n>.jpg` (ADR 0007: media bytes
//! are not RPC), which, like the Version itself, is served to anyone with a session.

use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Router,
    extract::{Path as UrlPath, Request, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use image::{ImageError, RgbImage, codecs::jpeg::JpegEncoder, imageops};
use nama_proto::nama::v1 as proto;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{Probe, Probing, RunningPlans, TrackKind, Tracks};
use crate::{
    db::Database,
    ffmpeg::{Ffmpeg, ReadOutError},
    images::{ImageStore, PREVIEW_FRAMES_DIR as DIR},
    library::{Contents, Libraries, LibraryError, Stored},
};

/// How far apart the stills are, in seconds.
const INTERVAL_S: u32 = 10;

/// How wide each still is, in pixels; its height keeps the video's aspect ratio.
const WIDTH: u32 = 320;

/// How many stills a sheet holds across and down.
const COLUMNS: u32 = 10;
const ROWS: u32 = 10;

/// The JPEG quality of each sheet.
const QUALITY: u8 = 80;

/// How often a Version waiting for playback to stop checks again.
const PLAYING_CHECK: Duration = Duration::from_secs(1);

/// Why a Version's Preview frames couldn't be made or kept, which says nothing about its file, so
/// they're tried again on the next pass.
#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error(transparent)]
    Ffmpeg(ReadOutError),
    #[error("couldn't keep them: {0}")]
    Store(#[from] io::Error),
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Makes Preview frames in the background, and reads what was made.
#[derive(Debug, Clone)]
pub(crate) struct PreviewFrames {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    db: Arc<Database>,
    contents: Contents,
    libraries: Libraries,
    tracks: Tracks,
    images: ImageStore,
    ffmpeg: Arc<Ffmpeg>,
    /// Whether anything plays, which the background work waits out.
    running: RunningPlans,
    /// Says when a probe pass has finished, after which there may be Versions to make them for.
    probing: Probing,
    /// The background loop, so shutting down can wait for it.
    tasks: TaskTracker,
    cancel: CancellationToken,
}

/// The sheets made of a Version's stills, and the stills' size and count.
struct Packed {
    sheets: Vec<Vec<u8>>,
    width: u32,
    height: u32,
    count: usize,
}

impl PreviewFrames {
    /// Makes them with `ffmpeg` and keeps their sheets in `images`, waiting while `running` plays
    /// anything, after each of `probing`'s passes.
    pub(crate) fn new(
        db: Arc<Database>,
        images: ImageStore,
        ffmpeg: Arc<Ffmpeg>,
        running: RunningPlans,
        probing: Probing,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                contents: Contents::new(Arc::clone(&db)),
                libraries: Libraries::new(Arc::clone(&db)),
                tracks: Tracks::new(Arc::clone(&db)),
                db,
                images,
                ffmpeg,
                running,
                probing,
                tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
            }),
        }
    }

    /// Makes the Preview frames of the Versions that need them in the background, now and after
    /// each probe pass, until [`Self::shutdown`]. Does nothing on a Server without ffmpeg.
    pub(crate) fn start(&self) {
        if self.inner.ffmpeg.ffmpeg.is_err() {
            return;
        }
        let frames = self.clone();
        self.inner.tasks.spawn(async move {
            let cancel = frames.inner.cancel.clone();
            cancel.run_until_cancelled(frames.keep_making()).await;
        });
    }

    /// Stops the background work, ffmpeg included, and waits for it.
    pub(crate) async fn shutdown(&self) {
        self.inner.cancel.cancel();
        self.inner.tasks.close();
        self.inner.tasks.wait().await;
    }

    /// Makes those needed now, then again after each probe pass, until cancelled.
    #[expect(
        clippy::infinite_loop,
        reason = "it makes them for as long as the Server serves, which cancels it"
    )]
    async fn keep_making(&self) {
        loop {
            if let Err(err) = self.make_needed().await {
                tracing::error!("could not read which Versions need Preview frames: {err}");
            }
            self.inner.probing.pass_finished().await;
        }
    }

    /// Makes the Preview frames of each Version that needs them, one at a time, only while
    /// nothing plays: a plan starting stops ffmpeg, and the Version is made again once nothing
    /// plays. A Version they can't be made for is logged and left for the next pass.
    async fn make_needed(&self) -> Result<(), Error> {
        for version in self.needed().await? {
            let made = loop {
                self.until_playing(false).await;
                tokio::select! {
                    made = self.make(version) => break made,
                    () = self.until_playing(true) => {}
                }
            };
            if let Err(err) = made {
                tracing::error!(version, "could not make a Version's Preview frames: {err}");
            }
        }
        Ok(())
    }

    /// Returns once whether any plan plays is `playing`.
    async fn until_playing(&self, playing: bool) {
        while self.inner.running.any_playing() != playing {
            tokio::time::sleep(PLAYING_CHECK).await;
        }
    }

    /// The Versions in Libraries that make Preview frames, probed with a video Track, whose file
    /// as it is now has none made, nor failed to have them made.
    // ponytail: whole-Library read of every such Library's files and probes after each probe
    // pass; ask only the Library probed once Libraries get big.
    pub(super) async fn needed(&self) -> Result<Vec<i64>, Error> {
        let kept: HashSet<(i64, i64, i64)> =
            sqlx::query!("SELECT version_id, size, modified_ns FROM preview_frames")
                .fetch_all(self.inner.db.reader())
                .await?
                .into_iter()
                .map(|row| (row.version_id, row.size, row.modified_ns))
                .collect();
        let mut needed = Vec::new();
        let libraries = self.inner.libraries.list().await?;
        for library in libraries.iter().filter(|library| library.preview_frames) {
            let files: Vec<Stored> = (self.inner.contents.stored(library.id).await?)
                .into_values()
                .collect();
            let probes = self.inner.tracks.of(&files).await?;
            needed.extend(
                (files.iter())
                    .filter(|file| !kept.contains(&(file.id, file.size, file.modified_ns)))
                    .filter(|file| probes.get(&file.id).is_some_and(has_video))
                    .map(|file| file.id),
            );
        }
        Ok(needed)
    }

    /// Makes the Version `version`'s Preview frames of its file as it is now, replacing those made
    /// of it before, and records them, or that ffmpeg couldn't make them of a file it could open.
    /// Does nothing for a Version there isn't, whose Library doesn't make them, or whose file as
    /// it is now has them made or failed already.
    pub(super) async fn make(&self, version: i64) -> Result<(), Error> {
        let contents = &self.inner.contents;
        let (Some((path, file)), Some(library)) = (
            contents.version_file(version).await?,
            contents.library_of_version(version).await?,
        ) else {
            return Ok(());
        };
        if !self.inner.libraries.get(library).await?.preview_frames
            || self.made_of(file).await?.is_some()
        {
            return Ok(());
        }
        self.inner
            .images
            .delete_dir(&[DIR, &version.to_string()])
            .await?;
        let stills = tempfile::tempdir()?;
        let packed = match (self.inner.ffmpeg)
            .keyframe_stills(&path, INTERVAL_S, WIDTH, stills.path())
            .await
        {
            Ok(()) => {
                let dir = stills.path().to_owned();
                tokio::task::spawn_blocking(move || pack(&dir))
                    .await
                    .map_err(io::Error::from)?
                    .unwrap_or_else(|err| {
                        tracing::warn!(version, "could not pack a Version's Preview frames: {err}");
                        None
                    })
            }
            Err(err @ (ReadOutError::Unavailable | ReadOutError::Run(_))) => {
                return Err(Error::Ffmpeg(err));
            }
            // Like a probe's, a failure to open the file, as on a share gone, says nothing about it.
            Err(err @ ReadOutError::Failed(_)) if tokio::fs::File::open(&path).await.is_err() => {
                return Err(Error::Ffmpeg(err));
            }
            Err(err @ (ReadOutError::TimedOut(_) | ReadOutError::Failed(_))) => {
                tracing::warn!(
                    version,
                    "ffmpeg couldn't make a Version's Preview frames: {err}"
                );
                None
            }
        };
        let manifest = match packed {
            Some(packed) => {
                for (n, sheet) in packed.sheets.iter().enumerate() {
                    let key = sheet_key(version, file.size, file.modified_ns, n);
                    self.inner
                        .images
                        .write(&key.each_ref().map(String::as_str), sheet)
                        .await?;
                }
                Some(packed.manifest())
            }
            None => None,
        };
        self.record(file, manifest).await
    }

    /// Records `manifest` as what was made of `file`, or `None` as that making them failed,
    /// replacing what was recorded for its Version. A Version removed meanwhile records nothing.
    async fn record(
        &self,
        file: Stored,
        manifest: Option<proto::PreviewFrames>,
    ) -> Result<(), Error> {
        let field = |get: fn(&proto::PreviewFrames) -> u32| manifest.as_ref().map(get);
        let (interval_ms, width, height) = (
            field(|m| m.interval_ms),
            field(|m| m.width),
            field(|m| m.height),
        );
        let (columns, rows, frames) = (field(|m| m.columns), field(|m| m.rows), field(|m| m.count));
        let recorded = self
            .inner
            .db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT OR REPLACE INTO preview_frames
                             (version_id, size, modified_ns, interval_ms, width, height,
                              sheet_columns, sheet_rows, frames)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        file.id,
                        file.size,
                        file.modified_ns,
                        interval_ms,
                        width,
                        height,
                        columns,
                        rows,
                        frames,
                    )
                    .execute(&mut **tx)
                    .await
                    .map(drop)
                })
            })
            .await;
        match recorded {
            Err(sqlx::Error::Database(err)) if err.is_foreign_key_violation() => Ok(()),
            recorded => Ok(recorded?),
        }
    }

    /// What's recorded for `file` as it is now: `Some(None)` if making its Preview frames failed.
    async fn made_of(
        &self,
        file: Stored,
    ) -> Result<Option<Option<proto::PreviewFrames>>, sqlx::Error> {
        let row = sqlx::query!(
            "SELECT interval_ms, width, height, sheet_columns, sheet_rows, frames
             FROM preview_frames WHERE version_id = ? AND size = ? AND modified_ns = ?",
            file.id,
            file.size,
            file.modified_ns
        )
        .fetch_optional(self.inner.db.reader())
        .await?;
        let n = |stored: Option<i64>| stored.and_then(|n| u32::try_from(n).ok());
        Ok(row.map(|row| {
            Some(proto::PreviewFrames {
                interval_ms: n(row.interval_ms)?,
                width: n(row.width)?,
                height: n(row.height)?,
                columns: n(row.sheet_columns)?,
                rows: n(row.sheet_rows)?,
                count: n(row.frames)?,
                ..Default::default()
            })
        }))
    }

    /// The Version `version`'s Preview frames as the API's manifest has them, if they're made of
    /// its file as it is now.
    pub(crate) async fn of(
        &self,
        version: i64,
    ) -> Result<Option<proto::PreviewFrames>, sqlx::Error> {
        let Some((_, file)) = self.inner.contents.version_file(version).await? else {
            return Ok(None);
        };
        Ok(self.made_of(file).await?.flatten())
    }
}

impl Packed {
    fn manifest(&self) -> proto::PreviewFrames {
        proto::PreviewFrames {
            interval_ms: INTERVAL_S * 1000,
            width: self.width,
            height: self.height,
            columns: COLUMNS,
            rows: ROWS,
            count: u32::try_from(self.count).unwrap_or(u32::MAX),
            ..Default::default()
        }
    }
}

/// The key of sheet `n` of the Version `version`, made of its file at `size` and `modified_ns`.
/// The file is part of it, so a sheet made again of a changed file gets a new key, as a key
/// names its bytes (`src/images/AGENTS.md`).
fn sheet_key(version: i64, size: i64, modified_ns: i64, n: usize) -> [String; 4] {
    [
        DIR.to_owned(),
        version.to_string(),
        format!("{size}-{modified_ns}"),
        format!("{n}.jpg"),
    ]
}

fn has_video(probe: &Probe) -> bool {
    (probe.tracks.iter()).any(|track| matches!(track.kind, TrackKind::Video { .. }))
}

/// The stills ffmpeg wrote to `dir`, packed into JPEG sheets of [`COLUMNS`] by [`ROWS`], or `None`
/// if it wrote none.
fn pack(dir: &Path) -> Result<Option<Packed>, ImageError> {
    // ffmpeg numbers them from 1 in a folder of their own.
    let written = std::fs::read_dir(dir)?.count();
    let stills: Vec<PathBuf> = (1..=written)
        .map(|n| dir.join(format!("{n}.jpg")))
        .collect();
    let Some(first) = stills.first() else {
        return Ok(None);
    };
    let (width, height) = image::image_dimensions(first)?;
    let per_sheet = usize::try_from(COLUMNS * ROWS).unwrap_or(usize::MAX);
    let sheets = (stills.chunks(per_sheet))
        .map(|stills| sheet(stills, width, height))
        .collect::<Result<_, _>>()?;
    Ok(Some(Packed {
        sheets,
        width,
        height,
        count: stills.len(),
    }))
}

/// `stills`, each `width` by `height`, packed left to right then top to bottom into one sheet.
fn sheet(stills: &[PathBuf], width: u32, height: u32) -> Result<Vec<u8>, ImageError> {
    let mut sheet = RgbImage::new(width * COLUMNS, height * ROWS);
    for (place, still) in (0..).zip(stills) {
        let still = image::open(still)?.into_rgb8();
        let (x, y) = (place % COLUMNS * width, place / COLUMNS * height);
        imageops::replace(&mut sheet, &still, i64::from(x), i64::from(y));
    }
    let mut jpeg = Vec::new();
    sheet.write_with_encoder(JpegEncoder::new_with_quality(&mut jpeg, QUALITY))?;
    Ok(jpeg)
}

/// Serves `GET /media/versions/<id>/preview-frames/<n>.jpg`, 404 unless the Version has that
/// sheet. Whether the caller has a session is up to the layers the router is put behind.
pub(crate) fn router(frames: PreviewFrames) -> Router {
    Router::new()
        .route(
            "/media/versions/{version}/preview-frames/{sheet}",
            get(serve),
        )
        .with_state(frames)
}

async fn serve(
    State(frames): State<PreviewFrames>,
    UrlPath((version, sheet)): UrlPath<(i64, String)>,
    request: Request,
) -> Response {
    let Some(n) = (sheet.strip_suffix(".jpg")).and_then(|n| n.parse::<usize>().ok()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let made = sqlx::query!(
        "SELECT size, modified_ns FROM preview_frames WHERE version_id = ? AND frames IS NOT NULL",
        version
    )
    .fetch_optional(frames.inner.db.reader())
    .await;
    let key = match made {
        Ok(Some(made)) => sheet_key(version, made.size, made.modified_ns, n),
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {version}'s Preview frames: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let key = key.each_ref().map(String::as_str);
    match frames
        .inner
        .images
        .serve(&key, "image/jpeg", None, request)
        .await
    {
        Ok(mut response) => {
            // The URL stays when they're made again of a changed file, so a browser checks the
            // key's ETag each time rather than keep what it has for good.
            (response.headers_mut()).insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-cache"),
            );
            response
        }
        Err(err) => {
            tracing::error!("could not serve the Version {version}'s Preview frames: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
#[path = "preview_frames_tests.rs"]
mod tests;
