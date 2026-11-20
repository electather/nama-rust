//! Asking the enabled Recognisers a Library lists which title an Item names, once the title
//! sources' search has decided (ADR 0050's title level). A Recogniser only proposes ids; Matching
//! fetches the title they name from the Library's title sources, as it does an ID Hint's.

use std::{ffi::OsStr, path::Path, sync::Arc};

use nama_plugin_api::{
    PluginId,
    contract::{self as wire, TitleRecognised, TitleToRecognise},
    metadata::{ExternalId, ExternalIds},
    recogniser::Recogniser,
};

use super::Miss;
use crate::library::{Candidate, RecogniserFailed, Recognisers, Source, TitleKind, TitleSources};

/// What a search for an Item's title found, which the Recognisers are asked about.
#[derive(Debug)]
pub(super) struct Searched<'a> {
    pub(super) kind: TitleKind,
    /// The title searched for: a Show's without its country tag.
    pub(super) title: &'a str,
    pub(super) year: Option<i32>,
    /// The Item files' paths, relative to the Library's folder.
    pub(super) paths: &'a [String],
    /// What the search found in the end, best fit first.
    pub(super) candidates: &'a [Candidate],
}

/// The title a Recogniser named.
#[derive(Debug)]
pub(super) struct Recognised {
    /// Its id that some title source keeps or looks titles up by.
    pub(super) id: ExternalId,
    /// Every well-formed id the Recogniser gave it.
    pub(super) ids: ExternalIds,
    /// The sources to fetch it from, by `id`'s namespace.
    pub(super) routed: Vec<Source>,
}

/// The title the first of `recognisers` to name one by an id `metadata`'s sources serve names, or
/// `None` when none can tell. Ids no source keeps or looks titles up by are ignored.
///
/// # Errors
///
/// [`Miss::RecogniserFailed`] when a Recogniser fails, so the next scan asks again.
pub(super) async fn recognise(
    recognisers: &Recognisers,
    metadata: &TitleSources,
    searched: &Searched<'_>,
) -> Result<Option<Recognised>, Miss> {
    let asked = &searched.asked();
    for plugin in recognisers.listed() {
        let answered = recognisers
            .ask(plugin, |recogniser: Arc<dyn Recogniser>| async move {
                recogniser.recognise_title(asked).await
            })
            .await
            .map_err(|RecogniserFailed| Miss::RecogniserFailed)?;
        let Some(named) = answered.map(|recognised| well_formed(plugin, &recognised)) else {
            continue;
        };
        if let Some(recognised) = served(metadata, searched.kind, named).await {
            let id = &recognised.id;
            tracing::info!(%plugin, title = searched.title, %id, "a Recogniser named it");
            return Ok(Some(recognised));
        }
    }
    Ok(None)
}

impl Searched<'_> {
    /// This search as the contract asks a Recogniser about it.
    fn asked(&self) -> TitleToRecognise {
        let kind = match self.kind {
            TitleKind::Movie => wire::TitleKind::TITLE_KIND_MOVIE,
            TitleKind::Show => wire::TitleKind::TITLE_KIND_SHOW,
        };
        TitleToRecognise {
            kind: kind.into(),
            title: self.title.to_owned(),
            year: self.year,
            paths: self.paths.to_vec(),
            candidates: self.candidates.iter().map(candidate).collect(),
            ..Default::default()
        }
    }
}

/// The first of `named` that some of `metadata`'s sources fetch a title of `kind` by, with every
/// id in `named`.
async fn served(
    metadata: &TitleSources,
    kind: TitleKind,
    named: Vec<ExternalId>,
) -> Option<Recognised> {
    let ids: ExternalIds = named.iter().cloned().collect();
    for id in named {
        let routed = metadata.by_namespace(id.namespace(), kind.method()).await;
        if !routed.is_empty() {
            return Some(Recognised { id, ids, routed });
        }
    }
    None
}

/// The ids `recognised` names, in its order, leaving out any that isn't well formed.
fn well_formed(plugin: &PluginId, recognised: &TitleRecognised) -> Vec<ExternalId> {
    (recognised.ids.iter())
        .filter_map(
            |id| match ExternalId::new(&id.namespace, id.value.clone()) {
                Ok(id) => Some(id),
                Err(err) => {
                    tracing::warn!(%plugin, namespace = id.namespace, %err, "a Recogniser's id");
                    None
                }
            },
        )
        .collect()
}

/// `path` relative to the first of `folders` it is under, `/`-separated as the contract carries
/// it; `None` if it is under none.
pub(super) fn relative(path: &str, folders: &[String]) -> Option<String> {
    let under = folders
        .iter()
        .find_map(|folder| Path::new(path).strip_prefix(folder).ok())?;
    let parts: Option<Vec<&str>> = under.iter().map(OsStr::to_str).collect();
    Some(parts?.join("/"))
}

fn candidate(candidate: &Candidate) -> wire::Candidate {
    wire::Candidate {
        id: title_id(&candidate.id).into(),
        ids: candidate.ids.iter().map(|id| title_id(&id)).collect(),
        title: candidate.title.clone(),
        original_title: candidate.original_title.clone(),
        year: candidate.year,
        poster: candidate.poster.clone(),
        origin_country: candidate.origin_country.clone(),
        vote_count: candidate.vote_count,
        ..Default::default()
    }
}

fn title_id(id: &ExternalId) -> wire::TitleId {
    wire::TitleId {
        namespace: id.namespace().to_owned(),
        value: id.value().to_owned(),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "recognisers_tests.rs"]
mod tests;
