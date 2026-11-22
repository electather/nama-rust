//! Subtitles a Device shows itself: text ones served as WebVTT for a player's `<track>`, and PGS,
//! VobSub and ASS ones as the Track's own `.sup`, `.mks` or `.ass` for a Device that draws them,
//! with the fonts an ASS one's file carries (ADR 0033).
//!
//! Where a subtitle comes from is kept apart from how it's served. A [`Source`] is a subtitle
//! Track inside a Version's file, or a sidecar, a subtitle file beside it. [`Cache`] serves any
//! source's subtitle: it has the source make it the first time it's asked for, keeps it under
//! `subtitles/` in the data directory, and answers from there after. The fonts are cached beside
//! the subtitles, and go with them; a sidecar carries none (ADR 0033).
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
    SUBTITLE_FORMAT_UNSPECIFIED as UNSPECIFIED, SUBTITLE_FORMAT_VOBSUB as VOBSUB,
    SUBTITLE_FORMAT_WEBVTT as WEBVTT,
};

mod ass;
mod text_pass;

pub(crate) use text_pass::TextPasses;

use super::{
    StoredTracks, WholeFileReads,
    tracks::{PROBE_VERSION, Probe, Track, TrackKind},
};
use crate::{
    db::Database,
    ffmpeg::{BurnIn, Ffmpeg, RunError, TextSubtitle, Transcode},
    images::write_atomically,
    library::{Contents, Stored},
};

/// The version of how a source makes WebVTT, part of every cache key so what an older one made is
/// made again; when to bump it is in `AGENTS.md`.
const WEBVTT_VERSION: u32 = 3;

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
/// a PGS one's own, or a VobSub one's own if its file carries its header.
pub(super) fn served_as(track: &Track) -> Option<proto::SubtitleFormat> {
    if track.is_text_subtitle() {
        Some(WEBVTT)
    } else if track.is_pgs_subtitle() {
        Some(PGS)
    } else if track.is_vobsub_subtitle()
        && matches!(track.kind, TrackKind::Subtitle { header: true, .. })
    {
        // Without its `.idx` header, libbitsub 1.13.2 hangs loading it, or draws every cue as a
        // black box, where Burn-in's ffmpeg guesses a grey palette that stays legible. MKV and MP4
        // rips carry it; a DVD dump in `.mpg` or `.ts` may not.
        Some(VOBSUB)
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
    pub(super) tracks: StoredTracks,
    pub(super) ffmpeg: Arc<Ffmpeg>,
    pub(super) cache: Cache,
    /// Runs text passes, which read whole files, in the background.
    pub(super) reads: WholeFileReads,
}

impl Subtitles {
    /// Finds sources in `db`, reads them with `ffmpeg`, text passes on `reads`, and caches them
    /// in `cache`.
    pub(super) fn new(
        db: &Arc<Database>,
        ffmpeg: Arc<Ffmpeg>,
        cache: Cache,
        reads: WholeFileReads,
    ) -> Self {
        Self {
            contents: Contents::new(Arc::clone(db)),
            tracks: StoredTracks::new(Arc::clone(db)),
            ffmpeg,
            cache,
            reads,
        }
    }
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
    let probed = match probed(&subtitles, version).await {
        Ok(Some(probed)) => probed,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {version}'s Track {track}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some(source) = Source::find(&probed, track, format_of) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let key = source.cache_key();
    let served = match source.reading {
        // Already what's served for PGS, so neither ffmpeg nor the cache needs to copy it.
        Reading::Sup => tokio::fs::read(&source.path).await.map_err(Error::from),
        Reading::TextPass => subtitles.read_by_text_pass(&key, &probed).await,
        Reading::Ffmpeg => {
            let read = source.read(&subtitles.ffmpeg);
            subtitles.cache.get_or_make(&key, read).await
        }
    };
    match served {
        Ok(served) => ([(header::CONTENT_TYPE, source.content_type())], served).into_response(),
        Err(err) => {
            tracing::error!("could not serve the Version {version}'s Track {track}: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Serves the Version `version`'s font `font`, read out of its file by a text pass the first time.
async fn serve_font(
    State(subtitles): State<Subtitles>,
    UrlPath((version, font)): UrlPath<(i64, usize)>,
) -> Response {
    let probed = match probed(&subtitles, version).await {
        Ok(Some(probed)) => probed,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            tracing::error!("could not look up the Version {version}'s font {font}: {err}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    // Only what probing recorded as a font, never another attachment.
    let (_, file, probe) = &probed;
    if font >= probe.font_attachments.len() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let key = cache_key(*file, &font_name(font));
    match subtitles.read_by_text_pass(&key, &probed).await {
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
    Read(#[from] RunError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Kept from an earlier text pass, which isn't tried again until the file changes.
    #[error("reading the subtitles out failed before, and the file hasn't changed since: {0}")]
    FailedBefore(String),
    /// The text pass stopped without it, as when the Server shuts down.
    #[error("the subtitles weren't read out")]
    NotReadOut,
}

/// A subtitle Track, inside a Version's file or in a sidecar beside it.
struct Source {
    /// The Version's file as stored now, which its Tracks were read from.
    file: Stored,
    /// Its name in the cache, which changes when what it's read from does.
    name: String,
    /// The file it's read from.
    path: String,
    /// Its place among that file's subtitle Tracks, which is how ffmpeg picks it out, or `None`
    /// for a sidecar's one.
    subtitle: Option<usize>,
    served_as: proto::SubtitleFormat,
    /// ASS or SSA, whose dialogue ffmpeg would make WebVTT with their styling and drawings in.
    ass: bool,
    /// How it's read out.
    reading: Reading,
}

/// How a [`Source`] is read out.
enum Reading {
    /// A text Track inside the file, which a text pass reads out with the others.
    TextPass,
    /// A `.sup` sidecar served as PGS, which is already what's served.
    Sup,
    /// Copied or converted by ffmpeg.
    Ffmpeg,
}

impl Source {
    /// The Track `track` of the Version at `path`, stored as `file`, which probing read as
    /// `probe`, in the format `format_of` gives it: `None` if there's no such Track or it gives
    /// none.
    fn find(
        (path, file, probe): &(String, Stored, Probe),
        track: usize,
        format_of: fn(&Track) -> Option<proto::SubtitleFormat>,
    ) -> Option<Self> {
        let found = probe.tracks.get(track)?;
        let (name, path, subtitle) = match found.sidecar() {
            // A sidecar holds just its one subtitle, and changes apart from its video, so it's
            // cached by its own size and time. ponytail: what an edited sidecar's older copy made
            // stays cached until its video changes or goes; have `keep_only` throw it away if that
            // adds up.
            Some(sidecar) => (
                format!("{track}.{}.{}", sidecar.size, sidecar.modified_ns),
                sidecar.path.clone(),
                None,
            ),
            None => (
                track.to_string(),
                path.clone(),
                Some(probe.ffmpeg_index(track)?),
            ),
        };
        let served_as = format_of(found)?;
        Some(Self {
            served_as,
            ass: found.is_ass_subtitle(),
            reading: match found.sidecar() {
                Some(_) if served_as == PGS => Reading::Sup,
                None if found.is_text_subtitle() => Reading::TextPass,
                _ => Reading::Ffmpeg,
            },
            file: *file,
            name,
            path,
            subtitle,
        })
    }

    fn cache_key(&self) -> PathBuf {
        cache_key(self.file, &cached_name(&self.name, self.served_as))
    }

    const fn content_type(&self) -> &'static str {
        match self.served_as {
            // Neither PGS nor VobSub has a registered media type.
            PGS | VOBSUB => "application/octet-stream",
            // Nor has ASS: this is the one players and media servers use.
            ASS => "text/x-ssa; charset=utf-8",
            UNSPECIFIED | WEBVTT => "text/vtt; charset=utf-8",
        }
    }

    /// Reads it out of its file, as a sidecar or an image Track is: a PGS Track copied into
    /// `.sup`, the framing a Device's renderer reads, a VobSub one into Matroska, which carries its
    /// palette and size beside it, an ASS or SSA one as ASS for a Device that draws it, and any
    /// other as WebVTT.
    async fn read(&self, ffmpeg: &Ffmpeg) -> Result<Vec<u8>, Error> {
        let (path, subtitle) = (&self.path, self.subtitle);
        match self.served_as {
            PGS => Ok(ffmpeg.subtitle(path, subtitle, "copy", "sup").await?),
            VOBSUB => Ok(ffmpeg.subtitle(path, subtitle, "copy", "matroska").await?),
            ASS => Ok(ffmpeg.subtitle(path, subtitle, "ass", "ass").await?),
            UNSPECIFIED | WEBVTT if self.ass => {
                let ass = ffmpeg.subtitle(path, subtitle, "ass", "ass").await?;
                Ok(ass::to_webvtt(&String::from_utf8_lossy(&ass)).into_bytes())
            }
            UNSPECIFIED | WEBVTT => Ok(ffmpeg.subtitle(path, subtitle, "webvtt", "webvtt").await?),
        }
    }
}

/// What the subtitle named `name` is cached as, served as `served_as`.
fn cached_name(name: &str, served_as: proto::SubtitleFormat) -> String {
    match served_as {
        // Read out as they are, so how WebVTT is made doesn't change them.
        PGS => format!("{name}.sup"),
        VOBSUB => format!("{name}.mks"),
        ASS => format!("{name}.ass"),
        UNSPECIFIED | WEBVTT => format!("{name}.v{WEBVTT_VERSION}.vtt"),
    }
}

/// What the font `font`, by its place among the fonts a Version's file carries, is cached as,
/// in the folder [`fonts_key`] names.
fn font_name(font: usize) -> String {
    format!("fonts/{font}")
}

/// Where the file `name`, cached for the Version stored as `file`, goes in the [`Cache`].
fn cache_key(file: Stored, name: &str) -> PathBuf {
    Path::new("versions").join(cache_folder(file)).join(name)
}

/// Where the fonts cached for the Version stored as `file` go in the [`Cache`]: a folder apart from
/// its subtitles, as libass's `fontsdir` reads every file in the one it's given.
fn fonts_key(file: Stored) -> PathBuf {
    cache_key(file, "fonts")
}

/// The folder the subtitles cached for the Version stored as `file` go in, named so it changes
/// when the file or how it's probed does, and they're never served for any other.
fn cache_folder(file: Stored) -> String {
    format!(
        "{}.{}.{}.{PROBE_VERSION}",
        file.id, file.size, file.modified_ns
    )
}

/// Subtitles on disk, each made by its source the first time it's asked for, and a Version's text
/// ones and fonts all at once. It has no size limit.
// ponytail: every Version's text subtitles and fonts are cached ahead of time, wanted or not, and
// ASS fonts, 10–50 MB a Version, aren't shared between Versions; if that fills disks, share fonts
// by their content's hash, then throw away the least recently served.
#[derive(Debug, Clone)]
pub(crate) struct Cache {
    dir: PathBuf,
    /// A lock per subtitle being made, or per Version's folder for a text pass, so requests for it
    /// at once wait for one making rather than each read the file.
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

    /// The folder the subtitles cached for the Version stored as `file` go in.
    pub(super) fn folder(&self, file: Stored) -> PathBuf {
        self.dir.join("versions").join(cache_folder(file))
    }

    /// Waits for the one making, at a time, of what's cached in `folder`: one `start` starts and
    /// hands the lock it holds until it's done, unless one is running already. Whoever waits
    /// doesn't own it, so it goes on when they give up. Returns whether `start` started it.
    async fn after_making_all(
        &self,
        folder: &Path,
        start: impl FnOnce(tokio::sync::OwnedMutexGuard<()>),
    ) -> bool {
        let making_lock = Arc::clone(self.making_locks().entry(folder.to_owned()).or_default());
        let making = Arc::clone(&making_lock).try_lock_owned();
        let started = making.is_ok();
        if let Ok(making) = making {
            start(making);
        }
        drop(making_lock.lock().await);
        drop(making_lock);
        self.making_locks()
            .retain(|_, lock| Arc::strong_count(lock) > 1);
        started
    }

    /// `subtitle`, a Track of the Version stored as `file`, as a burn-in reads it from here, if
    /// its ASS and every font the file carries are cached; otherwise `None`, and the burn-in reads
    /// the whole file for them. Blocks on the disk, briefly, as it's asked each time ffmpeg starts.
    pub(super) fn cached_burn_in(
        &self,
        file: Stored,
        subtitle: &TextSubtitle,
    ) -> Option<TextSubtitle> {
        let &TextSubtitle::Stream { track, fonts, .. } = subtitle else {
            return None;
        };
        // As `Source::cache_key` names a Track of the file served as ASS.
        let ass = self.dir.join(cache_key(file, &format!("{track}.ass")));
        let fonts_dir = self.dir.join(fonts_key(file));
        let all_cached =
            ass.is_file() && (0..fonts).all(|font| fonts_dir.join(font.to_string()).is_file());
        if !all_cached {
            return None;
        }
        // A data directory that isn't UTF-8 can't go in a filtergraph.
        Some(TextSubtitle::Cached {
            ass: ass.into_os_string().into_string().ok()?,
            fonts: fonts_dir.into_os_string().into_string().ok()?,
        })
    }

    /// `attempt`, burning its text subtitle in from here if it's cached for the Version stored as
    /// `file`.
    pub(super) fn burning_in(&self, file: Stored, attempt: &Transcode) -> Transcode {
        let mut attempt = attempt.clone();
        if let Some(BurnIn::Text { subtitle, .. }) = &mut attempt.video.burn_in
            && let Some(cached) = self.cached_burn_in(file, subtitle)
        {
            *subtitle = cached;
        }
        attempt
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
#[path = "subtitles/subtitles_tests.rs"]
mod tests;
