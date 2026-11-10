//! Text subtitles, served as WebVTT for a player's `<track>`.
//!
//! Where a subtitle comes from is kept apart from how it's served. [`Embedded`] is the one source
//! so far, a text subtitle Track inside a Version's file. [`Cache`] serves any source's WebVTT: it
//! has the source make it the first time it's asked for, keeps it under `subtitles/` in the data
//! directory, and answers from there after. Subtitle files next to a video would be another source
//! served by the same [`Cache`].
//!
//! ASS and SSA are served as plain cues ([`ass`]); burning them in is what keeps their styling.
//!
//! Clients fetch them at `/media/versions/<id>/subtitles/<track>` (ADR 0007: media bytes are not
//! RPC), which, like the Version itself, is served to anyone with a session.

use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use axum::{
    Router,
    extract::{Path as UrlPath, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

mod ass;

use crate::{
    artwork::write_atomically,
    contents::{Contents, Stored},
    ffmpeg::{Ffmpeg, SubtitleError},
    tracks::{PROBE_VERSION, Tracks},
};

/// The version of how a source makes WebVTT, part of every cache key so what an older one made is
/// made again; when to bump it is in `AGENTS.md`.
const WEBVTT_VERSION: u32 = 2;

/// Where clients fetch the Version `version`'s Track `track`, by its place in the Version's
/// Tracks, as WebVTT.
pub(super) fn url(version: i64, track: usize) -> String {
    format!("/media/versions/{version}/subtitles/{track}")
}

/// What serving a subtitle needs: what finds its source and what reads it.
#[derive(Debug, Clone)]
pub(super) struct Subtitles {
    pub(super) contents: Contents,
    pub(super) tracks: Tracks,
    pub(super) ffmpeg: Arc<Ffmpeg>,
    pub(super) cache: Cache,
}

/// Serves `GET` [`url`]s, 404 unless the Track is a text subtitle of a Version. Whether the caller
/// has a session is up to the layers the router is put behind.
pub(super) fn router(subtitles: Subtitles) -> Router {
    Router::new()
        .route("/media/versions/{version}/subtitles/{track}", get(serve))
        .with_state(subtitles)
}

async fn serve(
    State(subtitles): State<Subtitles>,
    UrlPath((version, track)): UrlPath<(i64, usize)>,
) -> Response {
    let source = match Embedded::find(&subtitles.contents, &subtitles.tracks, version, track).await
    {
        Ok(Some(source)) => source,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {version}'s Track {track}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let webvtt = subtitles
        .cache
        .webvtt(&source.cache_key(), source.webvtt(&subtitles.ffmpeg))
        .await;
    match webvtt {
        Ok(webvtt) => ([(header::CONTENT_TYPE, "text/vtt; charset=utf-8")], webvtt).into_response(),
        Err(err) => {
            tracing::error!("could not serve the Version {version}'s Track {track}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Why a subtitle couldn't be served.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("subtitle cache error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Read(#[from] SubtitleError),
}

/// A text subtitle Track inside a Version's file.
struct Embedded {
    /// The file as stored now, which its Tracks were read from.
    file: Stored,
    /// Its place in the Version's Tracks.
    track: usize,
    path: String,
    /// Its place among the file's subtitle Tracks, which is how ffmpeg picks it out.
    subtitle: usize,
    /// ASS or SSA, whose dialogue ffmpeg would make WebVTT with their styling and drawings in.
    ass: bool,
}

impl Embedded {
    /// The Version `version`'s Track `track`, `None` if there's no such Version, its Tracks weren't
    /// read from the file stored now, or the Track isn't a text subtitle.
    async fn find(
        contents: &Contents,
        tracks: &Tracks,
        version: i64,
        track: usize,
    ) -> Result<Option<Self>, sqlx::Error> {
        let Some((path, file)) = contents.version_file(version).await? else {
            return Ok(None);
        };
        // `of` leaves out Tracks read from what the file held before, which would pick the wrong
        // stream out of it now.
        let Some(probe) = tracks.of(&[file]).await?.remove(&version) else {
            return Ok(None);
        };
        let Some(found) = (probe.tracks.get(track)).filter(|found| found.is_text_subtitle()) else {
            return Ok(None);
        };
        let ass = matches!(found.codec.as_str(), "ass" | "ssa");
        Ok(Some(Self {
            file,
            track,
            path,
            subtitle: probe.subtitle_stream(track),
            ass,
        }))
    }

    fn cache_key(&self) -> PathBuf {
        Path::new("versions")
            .join(cache_folder(self.file))
            .join(format!("{}.v{WEBVTT_VERSION}.vtt", self.track))
    }

    async fn webvtt(&self, ffmpeg: &Ffmpeg) -> Result<Vec<u8>, Error> {
        if self.ass {
            let ass = ffmpeg.subtitle(&self.path, self.subtitle, "ass").await?;
            return Ok(ass::to_webvtt(&String::from_utf8_lossy(&ass)).into_bytes());
        }
        Ok(ffmpeg.subtitle(&self.path, self.subtitle, "webvtt").await?)
    }
}

/// The folder the subtitles cached for the Version stored as `file` go in, named so it changes
/// when the file or how it's probed does, and they're never served for any other.
fn cache_folder(file: Stored) -> String {
    format!(
        "{}.{}.{}.{PROBE_VERSION}",
        file.id, file.size, file.modified_ns
    )
}

/// WebVTT on disk, made by its source the first time it's asked for.
#[derive(Debug, Clone)]
pub(crate) struct Cache {
    dir: PathBuf,
    /// A lock per subtitle being made, so requests for it at once wait for one making rather
    /// than each read the file.
    making_locks: Arc<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>>,
}

impl Cache {
    /// The cache in `data_dir`.
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("subtitles"),
            making_locks: Arc::default(),
        }
    }

    /// The WebVTT cached at `key`, or else what `make` makes, which is cached there from then on.
    /// Of the requests for `key` at once, one makes it and the rest wait for it.
    async fn webvtt<E: From<io::Error>>(
        &self,
        key: &Path,
        make: impl Future<Output = Result<Vec<u8>, E>>,
    ) -> Result<Vec<u8>, E> {
        let path = self.dir.join(key);
        if let Some(webvtt) = cached(&path).await? {
            return Ok(webvtt);
        }
        let making_lock = Arc::clone(self.making_locks().entry(path.clone()).or_default());
        let webvtt = async {
            let _making = making_lock.lock().await;
            // Whoever held the lock before may have made it.
            if let Some(webvtt) = cached(&path).await? {
                return Ok(webvtt);
            }
            let webvtt = make.await?;
            write_atomically(&path, &webvtt).await?;
            Ok(webvtt)
        }
        .await;
        drop(making_lock);
        // Also drops the locks of makings cancelled midway, which nothing holds any more.
        self.making_locks()
            .retain(|_, lock| Arc::strong_count(lock) > 1);
        webvtt
    }

    /// Throws away the subtitles cached for every Version but `stored`, at its file as stored
    /// now: those of Versions gone, and of files changed or probed differently since.
    ///
    /// Nothing is served from what it misses or throws away too soon: a making still running
    /// caches under the file it read, which the next call throws away if that changed, and what's
    /// thrown away is made again.
    pub(crate) async fn keep_only(&self, stored: &[Stored]) -> io::Result<()> {
        let current: HashSet<String> = stored.iter().copied().map(cache_folder).collect();
        let mut cached = match tokio::fs::read_dir(self.dir.join("versions")).await {
            Ok(cached) => cached,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };
        while let Some(entry) = cached.next_entry().await? {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| current.contains(name))
            {
                continue;
            }
            match tokio::fs::remove_dir_all(entry.path()).await {
                // Thrown away by another call meanwhile.
                Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
                _ => {}
            }
        }
        Ok(())
    }

    fn making_locks(&self) -> MutexGuard<'_, HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>> {
        self.making_locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

async fn cached(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match tokio::fs::read(path).await {
        Ok(webvtt) => Ok(Some(webvtt)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        path::Path,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use super::Cache;

    #[tokio::test]
    async fn a_subtitle_is_made_by_its_source_once_then_served_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path());
        let key = Path::new("versions/7/2.vtt");

        let made = cache.webvtt(key, async { Ok::<_, io::Error>(b"WEBVTT\n".to_vec()) });
        let made = made.await.unwrap();
        let again = cache.webvtt(key, async { Err(io::Error::other("made again")) });
        let again = again.await.unwrap();

        assert_eq!(made, b"WEBVTT\n");
        assert_eq!(again, b"WEBVTT\n");
    }

    #[tokio::test]
    async fn two_first_requests_at_once_share_one_making() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path());
        let key = Path::new("versions/7/2.vtt");
        let makings = AtomicUsize::new(0);
        let make = || async {
            makings.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok::<_, io::Error>(b"WEBVTT\n".to_vec())
        };

        let (first, second) = tokio::join!(cache.webvtt(key, make()), cache.webvtt(key, make()));

        assert_eq!(first.unwrap(), b"WEBVTT\n");
        assert_eq!(second.unwrap(), b"WEBVTT\n");
        assert_eq!(makings.load(Ordering::SeqCst), 1);
    }
}
