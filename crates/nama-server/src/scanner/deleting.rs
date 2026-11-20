//! Deleting media: working out what a delete of a title takes ([`doomed`]), refusing it, asking
//! Fulfillers to stop monitoring it, deleting its files ([`files`]) under the Library's scan lock,
//! and what must follow (ADR 0038).

mod doomed;
mod files;

use std::{io, path::Path, path::PathBuf, sync::Arc};

use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfiller, Movies, Shows, StopMonitoring, stop_monitoring::Title},
    metadata::ExternalId,
};
use nama_proto::nama::v1::Request;

use doomed::Doomed;
pub(crate) use files::{File, read_only_folder};

use super::{Error, Scanner};
use crate::{
    library::{Change, LibraryEpisode, LibraryKind, TitleTarget, episode_name},
    plugins::{Failure, Retryable, ask_again},
    requests::{self, RequestTitle},
};

/// What a User or Plugin deletes.
#[derive(Debug)]
pub(crate) enum ToDelete {
    /// Every Version of a title of a Library: a Movie, or a Show's every Episode, a Season's or
    /// one Episode.
    Title { library: i64, title: TitleTarget },
    /// One Version of a Movie.
    Version(i64),
    /// One Version of an Episode of a Shows Library.
    EpisodeVersion {
        library: i64,
        episode: ExternalId,
        version: i64,
    },
}

/// An Episode a delete takes along, because a file of what it names holds it too, with its Show's
/// title.
#[derive(Debug, Clone)]
pub(crate) struct TakenAlong {
    pub(crate) show: String,
    pub(crate) episode: LibraryEpisode,
}

impl TakenAlong {
    /// As people name it, such as "Dark S01E02".
    pub(crate) fn name(&self) -> String {
        episode_name(&self.show, &self.episode)
    }

    /// The names of `taken_along`, as one list, such as "Dark S01E01, Dark S01E02".
    pub(crate) fn names(taken_along: &[Self]) -> String {
        let names: Vec<String> = taken_along.iter().map(Self::name).collect();
        names.join(", ")
    }
}

/// What a delete did, or a preview would do.
#[derive(Debug, Default)]
pub(crate) struct Deleted {
    pub(crate) files: Vec<File>,
    /// Each file that couldn't be deleted, and why.
    pub(crate) failed: Vec<(PathBuf, io::Error)>,
    /// The Versions whose own files went.
    versions: Vec<i64>,
    /// The name of each Fulfiller that couldn't be told to stop monitoring the title.
    pub(crate) fulfillers_not_told: Vec<String>,
    /// How many Playback Plans play the Versions, as a preview finds them.
    pub(crate) playing: usize,
    /// The Requests deleting would cancel, as a preview finds them.
    pub(crate) requests: Vec<Request>,
    /// The Episodes deleting would take along, as a preview finds them.
    pub(crate) taken_along: Vec<TakenAlong>,
    /// The Library folder the last scan found read-only that refuses the delete, as a preview
    /// finds it.
    pub(crate) read_only_folder: Option<String>,
}

impl Deleted {
    pub(crate) fn bytes_freed(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
}

impl Scanner {
    /// What [`Self::delete_media`] of `target` would delete, take along, stop playing and cancel,
    /// and the read-only folder that would refuse it, deleting nothing.
    pub(crate) async fn preview_delete(&self, target: &ToDelete) -> Result<Deleted, Error> {
        let doomed = self.doomed(target).await?;
        let versions = doomed.version_ids();
        let playing = self.inner.running.playing(&versions);
        let requests = (self.inner.requests)
            .waiting_on(&doomed.request_titles(&versions))
            .await?;
        let read_only_folder = doomed.read_only_folder().map(str::to_owned);
        let (doomed, planned) = tokio::task::spawn_blocking(move || {
            let planned = files::preview(&doomed);
            (doomed, planned)
        })
        .await?;
        Ok(Deleted {
            playing,
            requests,
            taken_along: doomed.taken_along,
            read_only_folder,
            ..planned
        })
    }

    /// Deletes `target` for `by`: asks every Fulfiller of its kind to stop monitoring it, then
    /// ends the plans playing it, deletes it from disk for good, removes the Versions whose files
    /// went as a scan removes them, with the images only their Matches stored, logs that `by`
    /// deleted them, and cancels the Requests still waiting on what they were of.
    ///
    /// Before any of that it refuses media in a folder the Library's last scan found read-only
    /// ([`Error::ReadOnly`]), then an Episode taken along that none of `also` names
    /// ([`Error::TakesAlongUnnamed`]). Once it holds the Library's scan lock it checks again, since
    /// a scan may have changed what the files hold, and refuses an Episode taken along that the
    /// first check didn't find, named or not, since the Fulfillers weren't told of it; a refusal
    /// there comes after the Fulfillers were told, and is logged. Once the files are deleted it returns what went: a
    /// later step's failure is only logged. The one way to delete media (see AGENTS.md, "One path
    /// for deleting media"). It holds the scan lock, so a scan racing it sees either everything or
    /// nothing gone, and finishes even if the caller stops waiting (ADR 0029). Shutting down
    /// waits for a delete already started.
    pub(crate) async fn delete_media(
        &self,
        target: ToDelete,
        also: Vec<ExternalId>,
        by: String,
    ) -> Result<Deleted, Error> {
        let doomed = self.doomed(&target).await?;
        doomed.check(&also)?;
        let deleting = (self.inner.tasks).spawn(self.clone().deleting(target, doomed, by));
        // The task is never aborted, so it only fails if it panicked.
        (deleting.await).unwrap_or_else(|_| Err(sqlx::Error::WorkerCrashed.into()))
    }

    /// Starts deleting `target` for the Plugin `plugin`, as [`Self::delete_media`] does, without
    /// waiting, and logs a failure naming `plugin`. Refuses as it does before starting, and returns
    /// the Episodes taken along. Started on the tracker before it returns, so shutting down right
    /// after still waits for the delete.
    pub(crate) async fn start_delete(
        &self,
        target: ToDelete,
        also: Vec<ExternalId>,
        by: String,
        plugin: PluginId,
    ) -> Result<Vec<TakenAlong>, Error> {
        let doomed = self.doomed(&target).await?;
        doomed.check(&also)?;
        let taken_along = doomed.taken_along.clone();
        let deleting = self.clone().deleting(target, doomed, by);
        self.inner.tasks.spawn(async move {
            if let Err(err) = deleting.await {
                tracing::error!(%plugin, "the Plugin's delete failed: {err}");
            }
        });
        Ok(taken_along)
    }

    /// The delete [`Self::delete_media`] describes, from stopping the monitoring of `checked`, what
    /// `target` took when it was checked, on. Run it on the tracker, so shutting down waits for it;
    /// it never reads the tracker's cancel token, so shutting down doesn't stop a delete halfway.
    async fn deleting(
        self,
        target: ToDelete,
        checked: Doomed,
        by: String,
    ) -> Result<Deleted, Error> {
        // First, so the service can't see the file missing and download it again (ADR 0038).
        let not_told = self.stop_monitoring(&checked).await;
        let library = checked.library.id;
        let _scan = self.inner.locks.lock(library).await;
        // Read again under the lock, which a scan merging a file into another Episode's holds.
        // Checked against what was checked before, which `also` named and the Fulfillers were told.
        let told = checked.taken_along_ids();
        let now = self.doomed(&target).await;
        let doomed = match now.and_then(|now| now.check(&told).map(|()| now)) {
            Ok(doomed) => doomed,
            Err(error) => {
                tracing::warn!(
                    %error,
                    by = %by,
                    title = %checked.title,
                    library,
                    "refused a delete once it held the scan lock, after the Fulfillers were told to \
                     stop monitoring it"
                );
                return Err(error);
            }
        };
        // Held until the Versions are gone, so none is served again before.
        let _ending = self
            .inner
            .running
            .end_for_deleting(&doomed.version_ids())
            .await;
        let folders: Vec<String> = (self.inner.libraries.list().await?.into_iter())
            .flat_map(|other| other.folders)
            .collect();
        let (doomed, mut deleted) = tokio::task::spawn_blocking(move || {
            let kept: Vec<&Path> = folders.iter().map(Path::new).collect();
            let deleted = files::delete(&doomed, &kept);
            (doomed, deleted)
        })
        .await?;
        deleted.fulfillers_not_told.extend(not_told);
        // Before anything else can fail: the files are gone for good.
        let files: Vec<_> = deleted.files.iter().map(|file| &file.path).collect();
        tracing::info!(
            by = %by,
            title = %doomed.title,
            library,
            ?files,
            bytes_freed = deleted.bytes_freed(),
            not_deleted = ?deleted.failed,
            "deleted media"
        );
        let removed = deleted.versions.iter().copied().map(Change::Remove);
        let events = match self.inner.contents.apply(library, removed.collect()).await {
            Ok(events) => events,
            Err(error) => {
                // The next scan removes them, as it does any file gone from disk.
                tracing::error!(
                    %error,
                    title = %doomed.title,
                    library,
                    "couldn't remove the deleted Versions; they stay listed until the next scan"
                );
                Vec::new()
            }
        };
        if let Some(probing) = &self.inner.probing {
            probing.clean_leftovers().await;
        }
        self.inner.events.publish(events);
        self.inner.artwork.drop_unused(&self.inner.contents).await;
        (self.cancel_requests(doomed.request_titles(&deleted.versions), by)).await;
        Ok(deleted)
    }

    /// Cancels the Requests still waiting on `titles`, whose media `by` deleted. A failure is
    /// logged, not returned: the media is gone by then, and an error would hide what went.
    async fn cancel_requests(&self, titles: Vec<RequestTitle>, by: String) {
        if let Err(error) = self.inner.requests.cancel_deleted(titles.clone(), by).await {
            tracing::error!(
                %error,
                ?titles,
                "couldn't cancel the Requests waiting on deleted media; they stay open"
            );
        }
    }

    /// Asks each enabled Fulfiller of `doomed`'s Library's kind to stop monitoring what it takes.
    /// Returns the name of each that couldn't be told, failing or panicking, which isn't asked
    /// again. One that says to try again is asked again later, in its scope (ADR 0047).
    async fn stop_monitoring(&self, doomed: &Doomed) -> Vec<String> {
        let Some(title) = doomed.to_unmonitor.clone() else {
            return Vec::new();
        };
        let title = StopMonitoring {
            title: Some(title),
            ..Default::default()
        };
        match doomed.library.kind {
            LibraryKind::Movies => self.ask_to_stop::<Movies>(&title, &doomed.title).await,
            LibraryKind::Shows => self.ask_to_stop::<Shows>(&title, &doomed.title).await,
        }
    }

    /// Asks each enabled Fulfiller of the kind `K` to stop monitoring `title`, which the log calls
    /// `name`, but one that needs an id the title lacks. Returns the name of each not told.
    async fn ask_to_stop<K: 'static>(&self, title: &StopMonitoring, name: &str) -> Vec<String> {
        let (kind, ids) = match &title.title {
            Some(Title::Movie(movie)) => ("Movie", &movie.ids[..]),
            Some(Title::Show(show)) => ("Show", &show.show_ids[..]),
            None => ("title", &[][..]),
        };
        let stop = |fulfiller: Arc<dyn Fulfiller<K>>| async move {
            let namespaces = ids.iter().map(|id| id.namespace.as_str());
            if let Some(needs) = requests::unmet(&*fulfiller, namespaces) {
                return Ok(Err(needs));
            }
            let stopped = fulfiller.stop_monitoring(title).await;
            stopped.map(Ok).map_err(Retryable)
        };
        let plugins = &self.inner.plugins;
        let mut not_told = Vec::new();
        for plugin in plugins.providers::<dyn Fulfiller<K>>() {
            let answer = plugins.ask(&plugin, &stop).await;
            // `None`: it panicked, which disabled it, or was disabled meanwhile.
            let reason = match answer.map(|answer| answer.result) {
                Some(Ok(Ok(()))) => continue,
                Some(Err(Failure {
                    kind: Some(wait), ..
                })) => {
                    let (id, name) = (plugin.clone(), name.to_owned());
                    let gave_up = move |reason: String| not_stopped(&id, &name, &reason);
                    let stop_again = <dyn Fulfiller<K>>::stop_monitoring;
                    ask_again(plugins, plugin, wait, title.clone(), stop_again, gave_up);
                    continue;
                }
                Some(Ok(Err(needs))) => requests::missing(&plugin, kind, &needs),
                Some(Err(failure)) => failure.message,
                None => "it panicked or was disabled".to_owned(),
            };
            not_stopped(&plugin, name, &reason);
            not_told.push(plugins.name(&plugin));
        }
        not_told
    }
}

/// Logs that the Plugin `plugin`'s Fulfiller didn't stop monitoring the deleted `name`, and why.
fn not_stopped(plugin: &PluginId, name: &str, reason: &str) {
    tracing::warn!(
        plugin = %plugin,
        title = name,
        reason,
        "a Fulfiller didn't stop monitoring a deleted title, so it may download it again"
    );
}

#[cfg(test)]
#[path = "deleting/deleting_tests.rs"]
mod tests;
