//! The text pass: one run of ffmpeg that reads every text subtitle Track and font out of a
//! Version's file into the [`Cache`](super::Cache), so the file is read once for all of them
//! rather than once for each (#829). The first fetch of any of them starts it in the background
//! and waits for it, so a fetch given up on, as by a reverse proxy, doesn't stop it. Image
//! subtitles stay read one at a time, when fetched, as caching them all could fill the disk.

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{ASS, Error, Subtitles, WEBVTT, ass, cached, cached_name, font_name};
use crate::{
    diagnostics,
    ffmpeg::{Ffmpeg, SubtitleError},
    images::write_atomically,
    library::Stored,
    playback::{probing::readable, tracks::Probe},
};

/// What a pass that would fail again leaves in the Version's cache folder, saying why, so none is
/// tried again until the file changes, and its cache folder with it.
const FAILED: &str = "failed";

/// What ffmpeg says when it couldn't write into the cache, which says nothing about the file it
/// reads, as each is an error only writing gives.
// ponytail: matches the C library's English messages, as in the Docker image; read ffmpeg's exit
// code instead if a Server in another locale remembers a full disk.
const CACHE_UNWRITABLE: &[&str] = &[
    "No space left on device",
    "Disk quota exceeded",
    "Read-only file system",
];

/// The folder in the Version's cache folder that ffmpeg writes into, as what it writes is whole
/// only once it's done. Its name starts with `.`, which no cached file's does.
const PARTIAL: &str = ".pass";

impl Subtitles {
    /// The text subtitle or font cached at `key` for the Version `probed` is of, by its path, its
    /// file as stored now and what probing read from it: as cached, or else once a text pass has
    /// read it out, which this starts unless one is running.
    pub(super) async fn read_by_text_pass(
        &self,
        key: &Path,
        probed: &(String, Stored, Probe),
    ) -> Result<Vec<u8>, Error> {
        let cached_at = self.cache.dir.join(key);
        if let Some(read) = cached(&cached_at).await? {
            return Ok(read);
        }
        let (_, file, _) = probed;
        let pass = TextPass::of(probed, self.cache.folder(*file), Arc::clone(&self.ffmpeg));
        let folder = pass.folder.clone();
        failed_before(&folder).await?;
        let span = diagnostics::following_current(tracing::info_span!(
            parent: None,
            "subtitle pass",
            file.id = file.id
        ));
        let start = |running| {
            let pass = async move {
                pass.run().await;
                drop(running);
            };
            self.probing.read_whole_file_in_background(span, pass);
        };
        self.cache.after_making_all(&folder, start).await;
        if let Some(read) = cached(&cached_at).await? {
            return Ok(read);
        }
        failed_before(&folder).await?;
        Err(Error::NotReadOut)
    }
}

/// [`Error::FailedBefore`] if a pass over the file whose cache folder is `folder` failed in a way
/// it would again.
async fn failed_before(folder: &Path) -> Result<(), Error> {
    match cached(&folder.join(FAILED)).await? {
        Some(why) => Err(Error::FailedBefore(
            String::from_utf8_lossy(&why).into_owned(),
        )),
        None => Ok(()),
    }
}

struct TextPass {
    path: String,
    /// Where what it reads out is cached.
    folder: PathBuf,
    ffmpeg: Arc<Ffmpeg>,
    tracks: Vec<TextTrack>,
    /// Each font by its place among the fonts, which names it in the cache, and among the file's
    /// attachments, which is how ffmpeg picks it out.
    fonts: Vec<(usize, usize)>,
}

/// A text subtitle Track inside a Version's file.
struct TextTrack {
    /// Its place among the Version's Tracks, which names it in the cache.
    track: usize,
    /// Its place among the file's subtitle Tracks, which is how ffmpeg picks it out.
    subtitle: usize,
    /// ASS or SSA, read out as ASS, served as it is and as the WebVTT made from it, since ffmpeg
    /// would make WebVTT with their styling and drawings in.
    ass: bool,
}

impl TextTrack {
    /// What it's cached as: WebVTT, and an ASS or SSA one as ASS too.
    fn cached_names(&self) -> Vec<String> {
        let name = self.track.to_string();
        let mut names = vec![cached_name(&name, WEBVTT)];
        if self.ass {
            names.push(cached_name(&name, ASS));
        }
        names
    }

    /// What ffmpeg encodes and writes it as.
    const fn format(&self) -> &'static str {
        if self.ass { "ass" } else { "webvtt" }
    }
}

impl TextPass {
    /// The pass over the Version at `path`, stored as `file`, which probing read as `probe`,
    /// caching into `folder`.
    fn of(
        (path, _, probe): &(String, Stored, Probe),
        folder: PathBuf,
        ffmpeg: Arc<Ffmpeg>,
    ) -> Self {
        let tracks = (probe.tracks.iter().enumerate())
            .filter(|(_, found)| found.is_text_subtitle())
            .filter_map(|(track, found)| {
                Some(TextTrack {
                    track,
                    // `None` for a sidecar, which isn't in the file.
                    subtitle: probe.ffmpeg_index(track)?,
                    ass: found.is_ass_subtitle(),
                })
            })
            .collect();
        Self {
            path: path.clone(),
            folder,
            ffmpeg,
            tracks,
            fonts: probe.font_attachments.iter().copied().enumerate().collect(),
        }
    }

    /// Reads out what isn't cached yet, unless it failed before, and logs why it couldn't.
    async fn run(self) {
        if let Err(err) = self.read_out_uncached().await {
            // Whoever started it may have stopped waiting, and so won't log it.
            tracing::warn!("could not read the subtitles out of {}: {err}", self.path);
        }
    }

    async fn read_out_uncached(&self) -> Result<(), Error> {
        failed_before(&self.folder).await?;
        let mut tracks = vec![];
        for track in &self.tracks {
            if !self.all_cached(track.cached_names()).await? {
                tracks.push(track);
            }
        }
        let mut fonts = vec![];
        for &font in &self.fonts {
            if !self.all_cached([font_name(font.0)]).await? {
                fonts.push(font);
            }
        }
        if tracks.is_empty() && fonts.is_empty() {
            return Ok(());
        }
        let partial = self.folder.join(PARTIAL);
        // Left by a pass the Server stopped midway.
        remove_dir_if_there(&partial).await?;
        tokio::fs::create_dir_all(&partial).await?;
        if !fonts.is_empty() {
            // Fonts sit in a folder of their own, which libass's `fontsdir` reads whole.
            tokio::fs::create_dir_all(partial.join("fonts")).await?;
            tokio::fs::create_dir_all(self.folder.join("fonts")).await?;
        }
        let read = self.read_out(&partial, &tracks, &fonts).await;
        if let Err(err) = remove_dir_if_there(&partial).await {
            tracing::warn!("could not throw away what ffmpeg wrote to {partial:?}: {err}");
        }
        read
    }

    async fn all_cached(&self, names: impl IntoIterator<Item = String>) -> io::Result<bool> {
        for name in names {
            if !tokio::fs::try_exists(self.folder.join(name)).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Reads `tracks` and `fonts` out with ffmpeg into `partial`, and caches them once they're
    /// all whole. If ffmpeg fails in a way it would again, remembers why.
    async fn read_out(
        &self,
        partial: &Path,
        tracks: &[&TextTrack],
        fonts: &[(usize, usize)],
    ) -> Result<(), Error> {
        let subtitles: Vec<_> = (tracks.iter())
            .map(|track| {
                let to = partial.join(track.track.to_string());
                (track.subtitle, track.format(), to)
            })
            .collect();
        let attachments: Vec<_> = (fonts.iter())
            .map(|&(font, attachment)| (attachment, partial.join(font_name(font))))
            .collect();
        if let Err(err) = self
            .ffmpeg
            .extract(&self.path, &subtitles, &attachments)
            .await
        {
            if fails_again(&err, &self.path).await {
                write_atomically(&self.folder.join(FAILED), err.to_string().as_bytes()).await?;
            }
            return Err(err.into());
        }
        for (track, (_, _, read)) in tracks.iter().zip(&subtitles) {
            let name = track.track.to_string();
            if track.ass {
                let ass = String::from_utf8_lossy(&tokio::fs::read(read).await?).into_owned();
                let webvtt = ass::to_webvtt(&ass);
                write_atomically(
                    &self.folder.join(cached_name(&name, WEBVTT)),
                    webvtt.as_bytes(),
                )
                .await?;
                keep(read, &self.folder.join(cached_name(&name, ASS))).await?;
            } else {
                keep(read, &self.folder.join(cached_name(&name, WEBVTT))).await?;
            }
        }
        for (&(font, _), (_, read)) in fonts.iter().zip(&attachments) {
            keep(read, &self.folder.join(font_name(font))).await?;
        }
        Ok(())
    }
}

/// Whether reading the file at `path` again would fail as it just did with `err`.
async fn fails_again(err: &SubtitleError, path: &str) -> bool {
    match err {
        // ffmpeg never reached the file.
        SubtitleError::Unavailable | SubtitleError::Run(_) => false,
        SubtitleError::Failed(why) if CACHE_UNWRITABLE.iter().any(|error| why.contains(error)) => {
            false
        }
        // ffmpeg fails fast on a file it can't open, as on a share that dropped, which may read
        // fine once it's back.
        SubtitleError::Failed(_) => readable(path).await,
        // Each try reads the whole file for up to an hour, as #850 says of keyframe listings.
        SubtitleError::TimedOut(_) => true,
    }
}

async fn keep(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (from.to_owned(), to.to_owned());
    tokio::task::spawn_blocking(move || {
        // Synced before the rename, or a crash can leave a whole name over an empty file.
        std::fs::File::open(&from)?.sync_all()?;
        std::fs::rename(from, to)
    })
    .await?
}

async fn remove_dir_if_there(dir: &Path) -> io::Result<()> {
    match tokio::fs::remove_dir_all(dir).await {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}
