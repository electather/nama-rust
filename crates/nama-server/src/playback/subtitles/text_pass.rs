//! The text pass: one run of ffmpeg that reads every text subtitle Track and font out of a
//! Version's file into the [`Cache`](super::Cache), so the file is read once for all of them
//! rather than once for each (#829). [`TextPasses`] runs it for each Version after Probing, one at
//! a time and only while nothing plays, so subtitles are cached before anyone turns them on
//! (#1649). A fetch of one not cached yet starts a pass in the background and waits for it, so a
//! fetch given up on, as by a reverse proxy, doesn't stop it. Image subtitles stay read one at a
//! time, when fetched, as caching them all could fill the disk.

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Notify;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{ASS, Error, Subtitles, WEBVTT, ass, cached, cached_name, font_name, probed};
use crate::{
    db::Database,
    diagnostics,
    ffmpeg::Ffmpeg,
    images::write_atomically,
    library::Stored,
    playback::{
        RunningPlans, WholeFileReads,
        read_failures::{Read, ReadFailure},
        tracks::Probe,
    },
};

/// What a pass that would fail again leaves in the Version's cache folder, saying why, so none is
/// tried again until the file changes, and its cache folder with it.
const FAILED: &str = "failed";

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
        let folder = self.cache.folder(probed.1);
        let mut ran_own = false;
        loop {
            if let Some(read) = cached(&cached_at).await? {
                return Ok(read);
            }
            failed_before(&folder).await?;
            if ran_own {
                return Err(Error::NotReadOut);
            }
            // A pass someone else started may have been stopped, as the background one is when
            // playback starts, so this one runs its own; playback doesn't stop that.
            ran_own = self.text_pass(probed, CancellationToken::new()).await;
        }
    }

    /// Runs a text pass over the Version `probed` is of in the background until `stop`, and
    /// waits for it, or else waits for the one running already. Returns whether it ran its own.
    async fn text_pass(
        &self,
        (path, file, probe): &(String, Stored, Probe),
        stop: CancellationToken,
    ) -> bool {
        let pass = TextPass::of(probe, self.cache.folder(*file), Arc::clone(&self.ffmpeg));
        let folder = pass.folder.clone();
        let path = path.clone();
        let span = diagnostics::following_current(tracing::info_span!(
            parent: None,
            "subtitle pass",
            file.id = file.id
        ));
        let start = |running| {
            let pass = async move {
                stop.run_until_cancelled(pass.run(&path)).await;
                drop(running);
            };
            self.reads.spawn(span, pass);
        };
        self.cache.after_making_all(&folder, start).await
    }
}

/// Runs the text pass of each Version that needs one in the background, after each probe pass.
#[derive(Debug, Clone)]
pub(crate) struct TextPasses {
    subtitles: Subtitles,
    /// Whether anything plays, which the background work waits out.
    running: RunningPlans,
    /// Notified after each probe pass.
    passed: Arc<Notify>,
    /// The background loop, so shutting down can wait for it.
    tasks: TaskTracker,
    cancel: CancellationToken,
}

impl TextPasses {
    /// Finds Versions in `db` and reads them with `ffmpeg` on `reads` into `cache`, each time
    /// `passed` is notified, waiting while `running` plays anything.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a separate handle the passes are built from; a struct bundling them \
                  would exist only for this call"
    )]
    pub(crate) fn new(
        db: &Arc<Database>,
        ffmpeg: Arc<Ffmpeg>,
        running: RunningPlans,
        cache: super::Cache,
        reads: WholeFileReads,
        passed: Arc<Notify>,
    ) -> Self {
        Self {
            subtitles: Subtitles::new(db, ffmpeg, cache, reads),
            running,
            passed,
            tasks: TaskTracker::new(),
            cancel: CancellationToken::new(),
        }
    }

    /// Runs the passes needed in the background, now and after each probe pass, until
    /// [`Self::shutdown`]. Does nothing on a Server without ffmpeg.
    pub(crate) fn start(&self) {
        if self.subtitles.ffmpeg.ffmpeg.is_err() {
            return;
        }
        let passes = self.clone();
        self.tasks.spawn(async move {
            let cancel = passes.cancel.clone();
            cancel.run_until_cancelled(passes.keep_reading()).await;
        });
    }

    /// Stops the background work, ffmpeg included, and waits for the loop; ffmpeg runs on
    /// [`WholeFileReads`], whose shutdown waits for it.
    pub(crate) async fn shutdown(&self) {
        self.cancel.cancel();
        self.tasks.close();
        self.tasks.wait().await;
    }

    #[expect(
        clippy::infinite_loop,
        reason = "it reads for as long as the Server serves, which cancels it"
    )]
    async fn keep_reading(&self) {
        loop {
            if let Err(err) = self.read_needed().await {
                tracing::error!("could not read which Versions need a text pass: {err}");
            }
            self.passed.notified().await;
        }
    }

    /// Runs the pass of each Version that needs one, one at a time.
    async fn read_needed(&self) -> Result<(), Error> {
        for version in self.needed().await? {
            if let Err(err) = self.pass_while_nothing_plays(version).await {
                tracing::error!(version, "could not run a Version's text pass: {err}");
            }
        }
        Ok(())
    }

    /// The Versions with a text subtitle Track in their file, whose file as it is now has neither
    /// had a pass finish nor fail in a way it would again.
    // ponytail: whole-Library read of every file and probe after each probe pass; ask only the
    // Library probed once Libraries get big.
    async fn needed(&self) -> Result<Vec<i64>, Error> {
        let Subtitles {
            contents,
            tracks,
            ffmpeg,
            cache,
            ..
        } = &self.subtitles;
        let files = contents.every_stored().await?;
        let probes = tracks.of(&files).await?;
        let mut needed = Vec::new();
        for file in files {
            let Some(probe) = probes.get(&file.id) else {
                continue;
            };
            if TextPass::of(probe, cache.folder(file), Arc::clone(ffmpeg))
                .is_needed()
                .await?
            {
                needed.push(file.id);
            }
        }
        Ok(needed)
    }

    /// Runs the Version `version`'s pass while nothing plays: playback starting stops it, and it
    /// runs again once nothing plays. A finished or failed pass makes it do nothing.
    async fn pass_while_nothing_plays(&self, version: i64) -> Result<(), Error> {
        loop {
            self.running.until_playing(false).await;
            let Some(probed) = probed(&self.subtitles, version).await? else {
                return Ok(());
            };
            let stop = CancellationToken::new();
            // Also stops it when shutting down drops this.
            let _stopping = stop.clone().drop_guard();
            let ran_own = tokio::select! {
                ran_own = self.subtitles.text_pass(&probed, stop) => ran_own,
                () = self.running.until_playing(true) => false,
            };
            // Having waited for someone else's, it can't tell whether that one finished.
            if ran_own {
                return Ok(());
            }
        }
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
    /// The pass over a Version which probing read as `probe`, caching into `folder`.
    fn of(probe: &Probe, folder: PathBuf, ffmpeg: Arc<Ffmpeg>) -> Self {
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
            folder,
            ffmpeg,
            tracks,
            fonts: probe.font_attachments.iter().copied().enumerate().collect(),
        }
    }

    /// Reads out of the file at `path` what isn't cached yet, unless it failed before, and logs
    /// why it couldn't.
    async fn run(self, path: &str) {
        if let Err(err) = self.read_out_uncached(path).await {
            // Whoever started it may have stopped waiting, and so won't log it.
            tracing::warn!("could not read the subtitles out of {path}: {err}");
        }
    }

    /// Whether it has text subtitle Tracks to read out that aren't cached, and hasn't failed in a
    /// way it would again.
    async fn is_needed(&self) -> io::Result<bool> {
        if self.tracks.is_empty() || tokio::fs::try_exists(self.folder.join(FAILED)).await? {
            return Ok(false);
        }
        let (tracks, fonts) = self.uncached().await?;
        Ok(!tracks.is_empty() || !fonts.is_empty())
    }

    /// The Tracks and fonts not all cached yet.
    async fn uncached(&self) -> io::Result<(Vec<&TextTrack>, Vec<(usize, usize)>)> {
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
        Ok((tracks, fonts))
    }

    async fn read_out_uncached(&self, path: &str) -> Result<(), Error> {
        failed_before(&self.folder).await?;
        let (tracks, fonts) = self.uncached().await?;
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
        let read = self.read_out(path, &partial, &tracks, &fonts).await;
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

    /// Reads `tracks` and `fonts` out of the file at `path` with ffmpeg into `partial`, and caches
    /// them once they're all whole. If ffmpeg fails in a way it would again, remembers why.
    async fn read_out(
        &self,
        path: &str,
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
        if let Err(err) = self.ffmpeg.extract(path, &subtitles, &attachments).await {
            if ReadFailure::of(&err, Read::WholeFile, path).await == ReadFailure::Lasting {
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

async fn keep(from: &Path, to: &Path) -> io::Result<()> {
    let (from, to) = (from.to_owned(), to.to_owned());
    tokio::task::spawn_blocking(move || {
        // Synced before the rename, or a crash can leave a whole name over an empty file. Windows
        // syncs only a file opened for writing.
        (std::fs::OpenOptions::new().write(true).open(&from)?).sync_all()?;
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
