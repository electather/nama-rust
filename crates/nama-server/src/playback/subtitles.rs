//! Subtitles a Device shows itself: text ones served as WebVTT for a player's `<track>`, and PGS
//! and ASS ones as the Track's own `.sup` or `.ass` for a Device that draws them, with the fonts
//! an ASS one's file carries (ADR 0033).
//!
//! Where a subtitle comes from is kept apart from how it's served. [`Embedded`] is the one source
//! so far, a subtitle Track inside a Version's file. [`Cache`] serves any source's subtitle: it
//! has the source make it the first time it's asked for, keeps it under `subtitles/` in the data
//! directory, and answers from there after. Subtitle files next to a video would be another source
//! served by the same [`Cache`]. The fonts are cached beside the subtitles, and go with them.
//!
//! As WebVTT, ASS and SSA are plain cues ([`ass`]); drawing or burning them in is what keeps
//! their styling.
//!
//! Clients fetch them at `/media/versions/<id>/subtitles/<track>`, an ASS one with its styling at
//! `…/<track>/ass`, and fonts at `/media/versions/<id>/fonts/<font>` (ADR 0007: media bytes are
//! not RPC), which, like the Version itself, are served to anyone with a session.

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
use nama_proto::nama::v1 as proto;
use proto::SubtitleFormat::{
    SUBTITLE_FORMAT_ASS as ASS, SUBTITLE_FORMAT_PGS as PGS,
    SUBTITLE_FORMAT_UNSPECIFIED as UNSPECIFIED, SUBTITLE_FORMAT_WEBVTT as WEBVTT,
};

mod ass;

use crate::{
    contents::{Contents, Stored},
    ffmpeg::{Ffmpeg, SubtitleError},
    images::write_atomically,
    tracks::{PROBE_VERSION, Probe, Track, Tracks},
};

/// The version of how a source makes WebVTT, part of every cache key so what an older one made is
/// made again; when to bump it is in `AGENTS.md`.
const WEBVTT_VERSION: u32 = 2;

/// Where clients fetch the Version `version`'s Track `track`, by its place in the Version's
/// Tracks, in `format`: the one it's [`served_as`], or ASS for an ASS or SSA one.
pub(super) fn url(version: i64, track: usize, format: proto::SubtitleFormat) -> String {
    let url = format!("/media/versions/{version}/subtitles/{track}");
    if format == ASS {
        format!("{url}/ass")
    } else {
        url
    }
}

/// Where clients fetch the Version `version`'s font `font`, by its place among the fonts its
/// file carries.
pub(super) fn font_url(version: i64, font: usize) -> String {
    format!("/media/versions/{version}/fonts/{font}")
}

/// The format `track` is served in at its plain [`url`], if it's served: WebVTT for a text one, or
/// a PGS one's own.
pub(super) fn served_as(track: &Track) -> Option<proto::SubtitleFormat> {
    if track.is_text_subtitle() {
        Some(WEBVTT)
    } else if track.is_pgs_subtitle() {
        Some(PGS)
    } else {
        None
    }
}

/// ASS, if `track` is an ASS or SSA one, which its [`url`] in ASS serves with its styling.
fn served_with_styling(track: &Track) -> Option<proto::SubtitleFormat> {
    track.is_ass_subtitle().then_some(ASS)
}

/// What serving a subtitle needs: what finds its source and what reads it.
#[derive(Debug, Clone)]
pub(super) struct Subtitles {
    pub(super) contents: Contents,
    pub(super) tracks: Tracks,
    pub(super) ffmpeg: Arc<Ffmpeg>,
    pub(super) cache: Cache,
}

/// Serves `GET` [`url`]s, 404 unless the Track is a subtitle of a Version served in that format,
/// and [`font_url`]s, 404 unless probing recorded the attachment as a font. Whether the caller has
/// a session is up to the layers the router is put behind.
pub(super) fn router(subtitles: Subtitles) -> Router {
    Router::new()
        .route(
            "/media/versions/{version}/subtitles/{track}",
            get(|state, url| serve(state, url, served_as)),
        )
        .route(
            "/media/versions/{version}/subtitles/{track}/ass",
            get(|state, url| serve(state, url, served_with_styling)),
        )
        .route("/media/versions/{version}/fonts/{font}", get(serve_font))
        .with_state(subtitles)
}

/// Serves the Version `version`'s Track `track` in the format `format_of` gives it.
async fn serve(
    State(subtitles): State<Subtitles>,
    UrlPath((version, track)): UrlPath<(i64, usize)>,
    format_of: fn(&Track) -> Option<proto::SubtitleFormat>,
) -> Response {
    let source = match probed(&subtitles, version).await {
        Ok(probed) => probed.and_then(|probed| Embedded::find(probed, track, format_of)),
        Err(err) => {
            tracing::error!("could not look up the Version {version}'s Track {track}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some(source) = source else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let served = subtitles
        .cache
        .get_or_make(&source.cache_key(), source.read(&subtitles.ffmpeg))
        .await;
    match served {
        Ok(served) => ([(header::CONTENT_TYPE, source.content_type())], served).into_response(),
        Err(err) => {
            tracing::error!("could not serve the Version {version}'s Track {track}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Serves the Version `version`'s font `font`, read out of its file with ffmpeg the first time.
async fn serve_font(
    State(subtitles): State<Subtitles>,
    UrlPath((version, font)): UrlPath<(i64, usize)>,
) -> Response {
    let (path, file, probe) = match probed(&subtitles, version).await {
        Ok(Some(probed)) => probed,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {version}'s font {font}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // Only what probing recorded as a font, never another attachment.
    let Some(&attachment) = probe.font_attachments.get(font) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let key = cache_key(file, &format!("font-{font}"));
    let read = async { Ok::<_, Error>(subtitles.ffmpeg.attachment(&path, attachment).await?) };
    match subtitles.cache.get_or_make(&key, read).await {
        // The renderer reads a font's bytes whatever its type, and nothing should guess one.
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => {
            tracing::error!("could not serve the Version {version}'s font {font}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// The Version `version`'s path, its file as stored now and what probing read from it, `None` if
/// there's no such Version or what probing read wasn't read from the file stored now.
async fn probed(
    subtitles: &Subtitles,
    version: i64,
) -> Result<Option<(String, Stored, Probe)>, sqlx::Error> {
    let Some((path, file)) = subtitles.contents.version_file(version).await? else {
        return Ok(None);
    };
    // `of` leaves out what was read from what the file held before, which would pick the wrong
    // stream out of it now.
    let probe = subtitles.tracks.of(&[file]).await?.remove(&version);
    Ok(probe.map(|probe| (path, file, probe)))
}

/// Why a subtitle couldn't be served.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("subtitle cache error: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Read(#[from] SubtitleError),
}

/// A subtitle Track inside a Version's file.
struct Embedded {
    /// The file as stored now, which its Tracks were read from.
    file: Stored,
    /// Its place in the Version's Tracks.
    track: usize,
    path: String,
    /// Its place among the file's subtitle Tracks, which is how ffmpeg picks it out.
    subtitle: usize,
    served_as: proto::SubtitleFormat,
    /// ASS or SSA, whose dialogue ffmpeg would make WebVTT with their styling and drawings in.
    ass: bool,
}

impl Embedded {
    /// The Track `track` of the Version at `path`, stored as `file`, which probing read as
    /// `probe`, in the format `format_of` gives it: `None` if there's no such Track or it gives
    /// none.
    fn find(
        (path, file, probe): (String, Stored, Probe),
        track: usize,
        format_of: fn(&Track) -> Option<proto::SubtitleFormat>,
    ) -> Option<Self> {
        let found = probe.tracks.get(track)?;
        Some(Self {
            served_as: format_of(found)?,
            subtitle: probe.ffmpeg_index(track)?,
            ass: found.is_ass_subtitle(),
            file,
            track,
            path,
        })
    }

    fn cache_key(&self) -> PathBuf {
        let name = match self.served_as {
            // Read out as they are, so how WebVTT is made doesn't change them.
            PGS => format!("{}.sup", self.track),
            ASS => format!("{}.ass", self.track),
            UNSPECIFIED | WEBVTT => format!("{}.v{WEBVTT_VERSION}.vtt", self.track),
        };
        cache_key(self.file, &name)
    }

    const fn content_type(&self) -> &'static str {
        match self.served_as {
            // PGS has no registered media type.
            PGS => "application/octet-stream",
            // Nor has ASS: this is the one players and media servers use.
            ASS => "text/x-ssa; charset=utf-8",
            UNSPECIFIED | WEBVTT => "text/vtt; charset=utf-8",
        }
    }

    /// Reads it out of the file: a PGS Track copied into `.sup`, the framing a Device's renderer
    /// reads, an ASS or SSA one as ASS for a Device that draws it, and any other as WebVTT.
    async fn read(&self, ffmpeg: &Ffmpeg) -> Result<Vec<u8>, Error> {
        let (path, subtitle) = (&self.path, self.subtitle);
        match self.served_as {
            PGS => Ok(ffmpeg.subtitle(path, subtitle, "copy", "sup").await?),
            ASS => Ok(ffmpeg.subtitle(path, subtitle, "ass", "ass").await?),
            UNSPECIFIED | WEBVTT if self.ass => {
                let ass = ffmpeg.subtitle(path, subtitle, "ass", "ass").await?;
                Ok(ass::to_webvtt(&String::from_utf8_lossy(&ass)).into_bytes())
            }
            UNSPECIFIED | WEBVTT => Ok(ffmpeg.subtitle(path, subtitle, "webvtt", "webvtt").await?),
        }
    }
}

/// Where the file `name`, cached for the Version stored as `file`, goes in the [`Cache`].
fn cache_key(file: Stored, name: &str) -> PathBuf {
    Path::new("versions").join(cache_folder(file)).join(name)
}

/// The folder the subtitles cached for the Version stored as `file` go in, named so it changes
/// when the file or how it's probed does, and they're never served for any other.
fn cache_folder(file: Stored) -> String {
    format!(
        "{}.{}.{}.{PROBE_VERSION}",
        file.id, file.size, file.modified_ns
    )
}

/// Subtitles on disk, each made by its source the first time it's asked for. It has no size limit.
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

    /// The subtitle cached at `key`, or else what `make` makes, which is cached there from then
    /// on. Of the requests for `key` at once, one makes it and the rest wait for it.
    async fn get_or_make<E: From<io::Error>>(
        &self,
        key: &Path,
        make: impl Future<Output = Result<Vec<u8>, E>>,
    ) -> Result<Vec<u8>, E> {
        let path = self.dir.join(key);
        if let Some(subtitle) = cached(&path).await? {
            return Ok(subtitle);
        }
        let making_lock = Arc::clone(self.making_locks().entry(path.clone()).or_default());
        let subtitle = async {
            let _making = making_lock.lock().await;
            // Whoever held the lock before may have made it.
            if let Some(subtitle) = cached(&path).await? {
                return Ok(subtitle);
            }
            let subtitle = make.await?;
            write_atomically(&path, &subtitle).await?;
            Ok(subtitle)
        }
        .await;
        drop(making_lock);
        // Also drops the locks of makings cancelled midway, which nothing holds any more.
        self.making_locks()
            .retain(|_, lock| Arc::strong_count(lock) > 1);
        subtitle
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
        Ok(subtitle) => Ok(Some(subtitle)),
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

        let made = cache.get_or_make(key, async { Ok::<_, io::Error>(b"WEBVTT\n".to_vec()) });
        let made = made.await.unwrap();
        let again = cache.get_or_make(key, async { Err(io::Error::other("made again")) });
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

        let (first, second) = tokio::join!(
            cache.get_or_make(key, make()),
            cache.get_or_make(key, make())
        );

        assert_eq!(first.unwrap(), b"WEBVTT\n");
        assert_eq!(second.unwrap(), b"WEBVTT\n");
        assert_eq!(makings.load(Ordering::SeqCst), 1);
    }
}
