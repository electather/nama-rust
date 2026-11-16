//! Asking Recognisers what a file is that the name parser made no Item from, such as an Episode
//! numbered from the start of its Show (ADR 0050). A proposal becomes the file's Items, which
//! Matching then decides as it does a parsed name's; the Kernel only takes it as a hint.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use nama_plugin_api::{
    PluginId,
    contract::{
        FileRecognised, FileToRecognise, LibraryKind, RecognisedEpisodes, RecognisedMovie, TitleId,
        file_recognised::Recognised,
    },
    metadata::{ExternalId, ExternalIds, IMDB, TMDB, TVDB},
    recogniser::{Recogniser, RecogniserMethod},
};
use tokio_util::sync::CancellationToken;

use super::{Error, names::MAX_EPISODES_PER_FILE};
use crate::{
    library::contents::{Change, Item},
    library::libraries::Kind,
    plugins::ExtensionPoints,
};

/// A change a scan plans to a file.
#[derive(Debug)]
pub(super) struct Planned {
    pub(super) change: Change,
    /// For a file the name parser made no Item from, its path relative to the Library folder
    /// it was found in, which Recognisers are asked about.
    pub(super) unread: Option<PathBuf>,
}

/// The changes `planned` makes, with each unread file's Items as the first Recogniser serving
/// `recognise_file` proposes them, in the Admin's precedence. A file none proposes usable Items
/// for stays unparsed. A file a Recogniser fails on is left out, so the next scan reads it again:
/// a new file stays unstored, a changed one keeps its old record and a re-read one its old parser
/// version. A scan calls it under the Library's scan lock, as Matching asks title sources.
pub(super) async fn recognise(
    plugins: &ExtensionPoints,
    kind: Kind,
    planned: Vec<Planned>,
    cancel: &CancellationToken,
) -> Result<Vec<Change>, Error> {
    // ponytail: asks in the Admin's precedence; a Library's own order of Recognisers comes later.
    let recognisers = plugins.ranked::<dyn Recogniser>();
    let mut changes = Vec::with_capacity(planned.len());
    for Planned { mut change, unread } in planned {
        let Some(relative) = unread.filter(|_| !recognisers.is_empty()) else {
            changes.push(change);
            continue;
        };
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        match ask(plugins, &recognisers, kind, &relative).await {
            Ok(Some(items)) => {
                set_items(&mut change, items);
                changes.push(change);
            }
            Ok(None) => changes.push(change),
            Err(()) => {}
        }
    }
    Ok(changes)
}

/// The Items the first of `recognisers` to propose usable ones gives the file at `relative`, or
/// `None` if none does. `Err` if one fails before any proposes, having logged why.
async fn ask(
    plugins: &ExtensionPoints,
    recognisers: &[PluginId],
    kind: Kind,
    relative: &Path,
) -> Result<Option<Vec<Item>>, ()> {
    let file = &FileToRecognise {
        path: slashed(relative),
        library_kind: match kind {
            Kind::Movies => LibraryKind::LIBRARY_KIND_MOVIES,
            Kind::Shows => LibraryKind::LIBRARY_KIND_SHOWS,
        }
        .into(),
        ..FileToRecognise::default()
    };
    for plugin in recognisers {
        let answered = plugins
            .ask(plugin, |recogniser: Arc<dyn Recogniser>| async move {
                if !recogniser.recognises(RecogniserMethod::File) {
                    return Ok(None);
                }
                recogniser.recognise_file(file).await.map(Some)
            })
            .await;
        // Disabled since the scan began, or it doesn't serve `recognise_file`.
        let Some(answer) = answered else { continue };
        let recognised = match answer.result {
            Ok(Some(recognised)) => recognised,
            Ok(None) => continue,
            Err(failure) => {
                tracing::warn!(
                    plugin = %answer.plugin,
                    path = file.path,
                    error = failure.message,
                    "a Recogniser failed, so the file is read again on the next scan"
                );
                return Err(());
            }
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
pub(super) fn items(kind: Kind, recognised: FileRecognised) -> Option<Vec<Item>> {
    match (kind, recognised.recognised?) {
        (Kind::Movies, Recognised::Movie(movie)) => movie_of(&movie).map(|movie| vec![movie]),
        (Kind::Shows, Recognised::Episodes(episodes)) => episodes_of(*episodes),
        (Kind::Movies, Recognised::Episodes(_)) | (Kind::Shows, Recognised::Movie(_)) => None,
    }
}

fn movie_of(movie: &RecognisedMovie) -> Option<Item> {
    let title = movie.title.trim();
    (!title.is_empty()).then(|| Item::Movie {
        title: title.to_owned(),
        year: movie.year.map(i64::from),
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
    let items = numbers.into_iter().map(|episode| Item::Episode {
        show: show.to_owned(),
        year: episodes.show_year.map(i64::from),
        hint: None,
        season: i64::from(episodes.season),
        episode: i64::from(episode),
    });
    Some(items.collect())
}

/// The ID Hint Matching goes by among `ids`, picked as the name parser picks one from a name:
/// TMDB's, else TVDB's, else IMDb's, else the first of any other namespace. Ids that aren't well
/// formed are left out.
fn hint(ids: &[TitleId]) -> Option<ExternalId> {
    let mut hints = ExternalIds::default();
    for id in ids {
        let Ok(id) = ExternalId::new(&id.namespace, id.value.clone()) else {
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
