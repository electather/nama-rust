//! Asking Recognisers what a file is that the name parser made no Item from, such as an Episode
//! numbered from the start of its Show (ADR 0050). A proposal becomes the file's Items, which
//! Matching then decides as it does a parsed name's; the Kernel only takes it as a hint.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use nama_plugin_api::{
    PluginId,
    contract::{
        self, FileRecognised, FileToRecognise, RecognisedEpisodes, RecognisedMovie, TitleId,
        file_recognised::Recognised,
    },
    metadata::{ExternalId, ExternalIds, IMDB, TMDB, TVDB},
    recogniser::{Recogniser, RecogniserMethod},
};
use tokio_util::sync::CancellationToken;

use super::{
    Error,
    names::{MAX_EPISODES_PER_FILE, YEARS, id_hint},
};
use crate::{
    db::Database,
    library::{Change, Contents, Item, Libraries, Library, LibraryKind},
    plugins::ExtensionPoints,
};

/// A change a scan plans to a file.
#[derive(Debug)]
pub(super) struct Planned {
    pub(super) change: Change,
    /// For a file the name parser made no Item from, its path relative to the Library folder
    /// it was found in, which Recognisers are asked about.
    pub(super) unread: Option<PathBuf>,
    /// Whether the file wasn't stored before, so the scan counts it as added, not updated.
    pub(super) added: bool,
}

/// The changes `planned` makes to a Library of `kind`, with each unread file's Items as the first
/// of the enabled Recognisers in the Library's `order` serving `recognise_file` proposes them. A
/// file none proposes usable Items for stays unparsed. A file a Recogniser fails on is left out,
/// so the next scan reads it again: a new file stays unstored, a changed one keeps its old record
/// and a re-read one its old parser version. Returns the changes to store and those left out. A
/// scan calls it under the Library's scan lock, as Matching asks title sources.
///
/// Each call's failure or success is recorded for the Admin to see, and each Recogniser that
/// failed is logged once, at the end, with how many files it failed on and its last error.
pub(super) async fn recognise(
    plugins: &ExtensionPoints,
    kind: LibraryKind,
    order: &[String],
    planned: Vec<Planned>,
    cancel: &CancellationToken,
) -> Result<(Vec<Change>, Vec<Planned>), Error> {
    let recognisers = listed(plugins, order);
    let mut changes = Vec::with_capacity(planned.len());
    let mut dropped = Vec::new();
    let mut failed = Failed::default();
    for mut file in planned {
        let Some(relative) = file.unread.as_ref().filter(|_| !recognisers.is_empty()) else {
            changes.push(file.change);
            continue;
        };
        if cancel.is_cancelled() {
            failed.log();
            return Err(Error::Cancelled);
        }
        match ask(plugins, &recognisers, kind, relative).await {
            Ok(Some(items)) => {
                set_items(&mut file.change, items);
                changes.push(file.change);
            }
            Ok(None) => changes.push(file.change),
            Err((plugin, message)) => {
                failed.add(plugin, message);
                dropped.push(file);
            }
        }
    }
    failed.log();
    Ok((changes, dropped))
}

/// The Recognisers that failed during one [`recognise`]: how many files each failed on, and its
/// last error.
#[derive(Default)]
struct Failed(BTreeMap<PluginId, (usize, String)>);

impl Failed {
    fn add(&mut self, plugin: PluginId, message: String) {
        let (files, last) = self.0.entry(plugin).or_default();
        *files = files.saturating_add(1);
        *last = message;
    }

    /// Logs each Recogniser that failed, once.
    fn log(&self) {
        for (plugin, (files, last)) in &self.0 {
            tracing::warn!(
                %plugin,
                files,
                last_error = last,
                "a Recogniser failed, so the files it failed on are read again on the next scan"
            );
        }
    }
}

/// Has Contents read `library`'s unparsed files again and forget what Matching left Unmatched
/// there if the Recognisers it lists that are enabled, in its order, aren't those its last scan
/// started with, and records them, in one write (ADR 0050); unchanged, it writes nothing. A scan
/// calls it under the Library's scan lock before it reads what is stored, so the files it marks
/// are read in that scan.
pub(super) async fn read_again_if_changed(
    db: &Database,
    libraries: &Libraries,
    plugins: &ExtensionPoints,
    library: &Library,
) -> Result<(), sqlx::Error> {
    let listed = listed(plugins, &library.recognisers);
    let read: Vec<&str> = listed.iter().map(PluginId::as_str).collect();
    let (id, read) = (library.id, read.join(","));
    // Read first, so a scan with nothing to read again doesn't wait behind other writes (#1130).
    if !libraries.recognisers_changed(id, &read).await? {
        return Ok(());
    }
    db.write(move |tx| {
        Box::pin(async move {
            if Libraries::record_recognisers_read(tx, id, &read).await? {
                Contents::read_unparsed_again(tx, id).await?;
                Contents::forget_unmatched(tx, id).await?;
            }
            Ok(())
        })
    })
    .await
}

/// Those of `order` that are enabled Recognisers, in its order.
pub(super) fn listed(plugins: &ExtensionPoints, order: &[String]) -> Vec<PluginId> {
    let enabled = plugins.providers::<dyn Recogniser>();
    (order.iter().map(|id| PluginId::from(id.clone())))
        .filter(|id| enabled.contains(id))
        .collect()
}

/// The Items the first of `recognisers` to propose usable ones gives the file at `relative`, or
/// `None` if none does. `Err` with the Recogniser and its error if one fails before any proposes.
/// Records each call that failed or answered.
async fn ask(
    plugins: &ExtensionPoints,
    recognisers: &[PluginId],
    kind: LibraryKind,
    relative: &Path,
) -> Result<Option<Vec<Item>>, (PluginId, String)> {
    let file = &FileToRecognise {
        path: slashed(relative),
        library_kind: match kind {
            LibraryKind::Movies => contract::LibraryKind::LIBRARY_KIND_MOVIES,
            LibraryKind::Shows => contract::LibraryKind::LIBRARY_KIND_SHOWS,
        }
        .into(),
        ..FileToRecognise::default()
    };
    for plugin in recognisers {
        let serves = plugins
            .read_provider(plugin, |recogniser: &dyn Recogniser| {
                recogniser.recognises(RecogniserMethod::File)
            })
            .await;
        if serves != Some(true) {
            continue;
        }
        let answered = plugins
            .ask(plugin, |recogniser: Arc<dyn Recogniser>| async move {
                recogniser.recognise_file(file).await
            })
            .await;
        // Disabled since the scan began.
        let Some(answer) = answered else { continue };
        let recognised = match answer.result {
            Ok(recognised) => recognised,
            Err(failure) => return Err((answer.plugin, failure.message)),
        };
        if recognised.recognised.is_none() {
            continue;
        }
        if let Some(items) = items(kind, recognised) {
            return Ok(Some(items));
        }
        tracing::warn!(
            plugin = %answer.plugin,
            path = file.path,
            "ignored a Recogniser's proposal that doesn't fit the Library"
        );
    }
    Ok(None)
}

/// `relative`, `/`-separated whatever the OS's separator, as the contract names files.
fn slashed(relative: &Path) -> String {
    let parts: Vec<_> = relative.iter().map(|part| part.to_string_lossy()).collect();
    parts.join("/")
}

fn set_items(change: &mut Change, recognised: Vec<Item>) {
    if let Change::Store { items, .. } | Change::Reparse { items, .. } = change {
        *items = recognised;
    }
}

/// The Items `recognised` proposes for a file in a Library of `kind`: a Movie only in a Movies
/// Library and Episodes only in a Shows Library, at most [`MAX_EPISODES_PER_FILE`] of them, as
/// the name parser would make them. `None` for a proposal that is empty or doesn't fit.
pub(super) fn items(kind: LibraryKind, recognised: FileRecognised) -> Option<Vec<Item>> {
    match (kind, recognised.recognised?) {
        (LibraryKind::Movies, Recognised::Movie(movie)) => {
            movie_of(&movie).map(|movie| vec![movie])
        }
        (LibraryKind::Shows, Recognised::Episodes(episodes)) => episodes_of(*episodes),
        (LibraryKind::Movies, Recognised::Episodes(_))
        | (LibraryKind::Shows, Recognised::Movie(_)) => None,
    }
}

fn movie_of(movie: &RecognisedMovie) -> Option<Item> {
    let title = movie.title.trim();
    (!title.is_empty()).then(|| Item::Movie {
        title: title.to_owned(),
        year: year(movie.year),
        hint: hint(&movie.ids),
    })
}

fn episodes_of(episodes: RecognisedEpisodes) -> Option<Vec<Item>> {
    let show = episodes.show_title.trim();
    // Sorted and unique, as one file can't hold an Episode twice.
    let numbers: BTreeSet<u32> = episodes.episodes.into_iter().collect();
    let fits = usize::try_from(MAX_EPISODES_PER_FILE).is_ok_and(|max| numbers.len() <= max);
    if show.is_empty() || numbers.is_empty() || !fits {
        return None;
    }
    let year = year(episodes.show_year);
    let items = numbers.into_iter().map(|episode| Item::Episode {
        show: show.to_owned(),
        year,
        hint: None,
        season: i64::from(episodes.season),
        episode: i64::from(episode),
    });
    Some(items.collect())
}

/// `year` if it is one a name may give, as the name parser keeps a year.
fn year(year: Option<i32>) -> Option<i64> {
    year.map(i64::from).filter(|year| YEARS.contains(year))
}

/// The ID Hint Matching goes by among `ids`, read and picked as the name parser reads and picks
/// one from a name: TMDB's, else TVDB's, else IMDb's, else the first of any other namespace. Ids
/// the name parser wouldn't read as a hint are left out.
fn hint(ids: &[TitleId]) -> Option<ExternalId> {
    let mut hints = ExternalIds::default();
    for id in ids {
        let Some(id) = id_hint(&id.namespace.to_ascii_lowercase(), &id.value) else {
            continue;
        };
        if hints.get(id.namespace()).is_none() {
            hints.insert(id);
        }
    }
    [TMDB, TVDB, IMDB]
        .into_iter()
        .find_map(|namespace| hints.get(namespace))
        .or_else(|| hints.iter().next())
}

#[cfg(test)]
#[path = "recognised_tests.rs"]
mod tests;
