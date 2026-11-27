//! Subtitles a Device shows itself: text ones served as WebVTT for a player's `<track>`, and PGS,
//! VobSub and ASS ones as the Track's own `.sup`, `.mks` or `.ass` for a Device that draws them,
//! with the fonts an ASS one's file carries (ADR 0033).
//!
//! Where a subtitle comes from is kept apart from how it's served. A [`Source`] is a subtitle
//! Track inside a Version's file, or a sidecar, a subtitle file beside it. [`Cache`] serves any
//! source's subtitle: it has the source make it the first time it's asked for, keeps it under
//! `subtitles/` in the data directory, and answers from there after. The fonts are cached beside
//! the subtitles, and go with them; a sidecar carries none (ADR 0033). A fetch that adds to the
//! cache past its limit, a Playback setting, deletes the subtitles of the Versions served least
//! recently (#2495).
//!
//! As WebVTT, ASS and SSA are plain cues ([`ass`]); drawing or burning them in is what keeps
//! their styling.
//!
//! Clients fetch them at `/media/versions/<id>/subtitles/<track>`, an ASS one with its styling at
//! `…/<track>/ass`, and fonts at `/media/versions/<id>/fonts/<font>` (ADR 0007: media bytes are
//! not RPC), which, like the Version itself, are served to anyone with a session.

use std::{
    collections::{HashMap, HashSet},
    fs::{File, FileTimes},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, SystemTime},
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
use walkdir::WalkDir;

mod ass;
mod text_pass;

pub(crate) use text_pass::TextPasses;

use super::{
    StoredTracks, WholeFileReads,
    tracks::{PROBE_VERSION, Probe, Track, TrackKind},
};
use crate::{
    db::{self, Database},
    ffmpeg::{BurnIn, Ffmpeg, RunError, TextSubtitle, Transcode},
    images::write_atomically,
    library::{Contents, Stored},
    server_settings::PlaybackSettings,
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

/// The format `track` is served in at its plain [`url()`], if it's served: WebVTT for a text one,
/// or a PGS one's own, or a VobSub one's own if its file carries its header.
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

/// ASS, if `track` is an ASS or SSA one, which its [`url()`] in ASS serves with its styling.
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
    /// Holds the cache's limit.
    pub(super) settings: PlaybackSettings,
}

impl Subtitles {
    /// Finds sources in `db`, reads them with `ffmpeg`, text passes on `reads`, and caches them
    /// in `cache`, within the limit `settings` holds.
    pub(super) fn new(
        db: &Arc<Database>,
        ffmpeg: Arc<Ffmpeg>,
        cache: Cache,
        reads: WholeFileReads,
        settings: PlaybackSettings,
    ) -> Self {
        Self {
            contents: Contents::new(Arc::clone(db)),
            tracks: StoredTracks::new(Arc::clone(db)),
            ffmpeg,
            cache,
            reads,
            settings,
        }
    }

    /// What `read` reads from the cache at `key`, marked as just served. If it wasn't cached
    /// before, deletes the subtitles served least recently until the cache fits its limit, as
    /// only a fetch does (`AGENTS.md`).
    async fn serve_cached(
        &self,
        key: &Path,
        read: impl Future<Output = Result<Vec<u8>, Error>>,
    ) -> Result<Vec<u8>, Error> {
        let path = self.cache.dir.join(key);
        let was_cached = tokio::fs::try_exists(&path).await?;
        let served = read.await?;
        mark_served(&path).await;
        if !was_cached {
            let limit = self.settings.subtitle_cache_bytes();
            if let Err(err) = self.cache.evict(limit, &path).await {
                tracing::error!("could not keep the subtitle cache within its limit: {err}");
            }
        }
        Ok(served)
    }

    /// What a text pass reads out to `key` for the Version `probed` is of, as
    /// [`Self::serve_cached`] serves it, in a task of its own: a fetch given up on, as by a reverse
    /// proxy, leaves the pass it started reading into the cache, so it still evicts once that's done.
    async fn serve_by_text_pass(
        &self,
        key: PathBuf,
        probed: (String, Stored, Probe),
    ) -> Result<Vec<u8>, Error> {
        let subtitles = self.clone();
        let serving = tokio::spawn(async move {
            let read = subtitles.read_by_text_pass(&key, &probed);
            subtitles.serve_cached(&key, read).await
        });
        serving.await.map_err(io::Error::from)?
    }
}

/// Serves `GET` [`url()`]s, 404 unless the Track is a subtitle of a Version served in that format,
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
        Reading::TextPass => subtitles.serve_by_text_pass(key, probed).await,
        Reading::Ffmpeg => {
            let read = subtitles
                .cache
                .get_or_make(&key, source.read(&subtitles.ffmpeg));
            subtitles.serve_cached(&key, read).await
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
    match subtitles.serve_by_text_pass(key, probed).await {
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

    /// Reads it out of its file, as a sidecar or an image Track fetched before its pass is, in
    /// its [`encoding`], but an ASS or SSA one served as WebVTT read as ASS and made WebVTT here.
    async fn read(&self, ffmpeg: &Ffmpeg) -> Result<Vec<u8>, Error> {
        let (path, subtitle) = (&self.path, self.subtitle);
        if self.ass && matches!(self.served_as, UNSPECIFIED | WEBVTT) {
            let ass = ffmpeg.subtitle(path, subtitle, "ass", "ass").await?;
            return Ok(ass::to_webvtt(&String::from_utf8_lossy(&ass)).into_bytes());
        }
        let (codec, muxer) = encoding(self.served_as);
        Ok(ffmpeg.subtitle(path, subtitle, codec, muxer).await?)
    }
}

/// The encoder and muxer ffmpeg reads a subtitle out in to serve it as `served_as`: a PGS one
/// copied into `.sup`, the framing a Device's renderer reads, a VobSub one into Matroska, which
/// carries its palette and size beside it, an ASS or SSA one as ASS, and any other as WebVTT.
const fn encoding(served_as: proto::SubtitleFormat) -> (&'static str, &'static str) {
    match served_as {
        PGS => ("copy", "sup"),
        VOBSUB => ("copy", "matroska"),
        ASS => ("ass", "ass"),
        UNSPECIFIED | WEBVTT => ("webvtt", "webvtt"),
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

/// Subtitles on disk, each made by its source the first time it's asked for, and a Version's
/// subtitles in its file and fonts all at once. What's cached for a Version is kept or deleted
/// as one, by when any of it was last served.
// ponytail: ASS fonts, 10–50 MB a Version, aren't shared between Versions; share them by their
// content's hash if they crowd image Tracks out of the limit.
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
        // A burn-in serves them too, so eviction keeps them while it may start again.
        if let Err(err) = touch(&ass) {
            tracing::warn!("could not mark {} as served: {err}", ass.display());
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

    /// The bytes the cache takes.
    ///
    /// # Errors
    ///
    /// If the cache can't be read.
    pub(crate) async fn size(&self) -> io::Result<u64> {
        let versions = self.dir.join("versions");
        let folders = tokio::task::spawn_blocking(move || version_folders(&versions)).await??;
        Ok(folders.iter().map(|(_, size, _)| size).sum())
    }

    /// Deletes the subtitles of the Versions served least recently until the cache takes at most
    /// `limit` bytes, never those of `served`'s Version, nor of one being made, whose making would
    /// fail, nor of one whose pass failed, which would then be read again.
    // ponytail: lists the whole cache after each fetch that made something, about a stat per
    // cached file; keep a running total if a cache of tens of thousands of files makes that slow.
    async fn evict(&self, limit: u64, served: &Path) -> io::Result<()> {
        let versions = self.dir.join("versions");
        let (cache, served) = (self.clone(), served.to_owned());
        tokio::task::spawn_blocking(move || {
            let folders = version_folders(&versions)?;
            evict_listed(folders, limit, |folder| {
                cache.delete_unless_kept(folder, &served)
            })
        })
        .await?
    }

    /// Deletes the Version's `folder` unless it holds `served`, a failed pass's mark or anything
    /// being made, and returns whether it did. It holds the folder's lock meanwhile, so a text pass
    /// starting then waits and runs after, rather than have its folder deleted midway, which would
    /// fail as if the file couldn't be read.
    fn delete_unless_kept(&self, folder: &Path, served: &Path) -> io::Result<bool> {
        if served.starts_with(folder) || folder.join(text_pass::FAILED).exists() {
            return Ok(false);
        }
        let making_lock = {
            let mut locks = self.making_locks();
            if (locks.keys()).any(|making| making != folder && making.starts_with(folder)) {
                return Ok(false);
            }
            Arc::clone(locks.entry(folder.to_owned()).or_default())
        };
        let deleted = match Arc::clone(&making_lock).try_lock_owned() {
            Ok(_deleting) => match std::fs::remove_dir_all(folder) {
                // Deleted meanwhile, as by `keep_only` or another fetch's eviction.
                Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
                _ => Ok(true),
            },
            Err(_) => Ok(false),
        };
        drop(making_lock);
        self.making_locks()
            .retain(|_, lock| Arc::strong_count(lock) > 1);
        deleted
    }

    fn making_locks(&self) -> MutexGuard<'_, HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>> {
        self.making_locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Marks the subtitle or font cached at `path` as just served, so its Version's are the last
/// [`Cache::evict`] deletes.
async fn mark_served(path: &Path) {
    let served = path.to_owned();
    let touched = tokio::task::spawn_blocking(move || touch(&served)).await;
    if let Err(err) = touched.map_err(io::Error::from).and_then(|touched| touched) {
        // Not marking it only lets eviction pick it sooner.
        tracing::warn!("could not mark {} as served: {err}", path.display());
    }
}

/// Marks the file at `path` as just served, unless it's gone, as by a delete of its Version or
/// another fetch's eviction.
fn touch(path: &Path) -> io::Result<()> {
    // Set explicitly, as mounts with `noatime` or `relatime` don't update it on every read. Opened
    // for writing, as Windows refuses to set a file's times through a read-only handle.
    let touched = File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_times(FileTimes::new().set_accessed(served_now())));
    match touched {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        touched => touched,
    }
}

/// Now, by the Server's clock (`AGENTS.md`), as a file's times are set.
fn served_now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(u64::try_from(db::now_ms()).unwrap_or_default())
}

/// A Version's folder in the cache, as when any of it was last served, the bytes it takes and its
/// path: in that order, so they sort least recently served first.
type Listed = (SystemTime, u64, PathBuf);

/// Each Version's folder in `versions`, leaving out what's deleted while it's listed.
fn version_folders(versions: &Path) -> io::Result<Vec<Listed>> {
    let gone =
        |err: &walkdir::Error| err.io_error().map(io::Error::kind) == Some(io::ErrorKind::NotFound);
    let mut folders = Vec::new();
    for folder in WalkDir::new(versions).min_depth(1).max_depth(1) {
        let folder = match folder {
            Ok(folder) => folder.into_path(),
            Err(err) if gone(&err) => continue,
            Err(err) => return Err(err.into()),
        };
        let (mut served, mut size) = (SystemTime::UNIX_EPOCH, 0);
        for file in WalkDir::new(&folder) {
            let metadata = match file.and_then(|file| file.metadata()) {
                Ok(metadata) if metadata.is_file() => metadata,
                Ok(_) => continue,
                Err(err) if gone(&err) => continue,
                Err(err) => return Err(err.into()),
            };
            // Accessed no later than written, it was read ahead and never served.
            let accessed = metadata.accessed()?;
            if accessed > metadata.modified()? {
                served = served.max(accessed);
            }
            size += metadata.len();
        }
        folders.push((served, size, folder));
    }
    Ok(folders)
}

/// [`Cache::evict`] from `folders`, the cache as listed, with `delete`, which says whether it
/// deleted the folder it's given or keeps it.
fn evict_listed(
    mut folders: Vec<Listed>,
    limit: u64,
    mut delete: impl FnMut(&Path) -> io::Result<bool>,
) -> io::Result<()> {
    let mut total: u64 = folders.iter().map(|(_, size, _)| size).sum();
    folders.sort_unstable();
    for (_, size, folder) in folders {
        if total <= limit {
            break;
        }
        if delete(&folder)? {
            total -= size;
        }
    }
    Ok(())
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
