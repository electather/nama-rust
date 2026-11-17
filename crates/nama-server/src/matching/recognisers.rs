//! Asking the enabled Recognisers a Library lists which title an Item names, once the title
//! sources' search has decided (ADR 0050's title level). A Recogniser only proposes ids; Matching
//! fetches the title they name from the Library's title sources, as it does an ID Hint's.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    mem,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use nama_plugin_api::{
    PluginId, PointKey,
    contract::{self as wire, TitleRecognised, TitleToRecognise},
    metadata::{ExternalId, ExternalIds},
    recogniser::{Recogniser, RecogniserMethod},
};

use super::Miss;
use crate::{
    library::{Candidate, Source, TitleKind, TitleSources},
    plugins::{Answer, ExtensionPoints},
};

/// The enabled Recognisers a Library lists.
#[derive(Debug, Clone, Default)]
pub(super) struct Recognisers {
    plugins: ExtensionPoints,
    /// The Plugins asked, first asked first: none until [`Recognisers::in_order`] gives some.
    order: Vec<PluginId>,
    /// Each Recogniser that failed since [`Recognisers::log_failures`] last ran: how many titles
    /// it failed on, and its last error.
    failed: Arc<Mutex<BTreeMap<PluginId, (usize, String)>>>,
}

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

impl Recognisers {
    pub(super) fn new(plugins: ExtensionPoints) -> Self {
        Self {
            plugins,
            order: Vec::new(),
            failed: Arc::default(),
        }
    }

    /// These Recognisers asking those of the Plugins `order` names that are enabled, in its
    /// order, as a Library's `recognisers` lists them, with no failures yet.
    pub(super) fn in_order(&self, order: &[String]) -> Self {
        Self {
            plugins: self.plugins.clone(),
            order: order.iter().map(|id| PluginId::from(id.clone())).collect(),
            failed: Arc::default(),
        }
    }

    /// Logs each Recogniser that failed since this last ran, once, with how many titles it
    /// failed on and its last error. Matching calls it as it stops asking during a scan.
    pub(super) fn log_failures(&self) {
        let failed = mem::take(&mut *self.failures());
        for (plugin, (titles, last)) in failed {
            tracing::warn!(
                %plugin,
                titles,
                last_error = last,
                "a Recogniser failed, so the titles it failed on are matched again on the next scan"
            );
        }
    }

    fn failures(&self) -> MutexGuard<'_, BTreeMap<PluginId, (usize, String)>> {
        self.failed.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The title the first Recogniser in order to name one by an id `metadata`'s sources serve
    /// names, or `None` when none can tell. Ids no source keeps or looks titles up by are ignored.
    ///
    /// # Errors
    ///
    /// [`Miss::RecogniserFailed`] when a Recogniser fails, so the next scan asks again.
    pub(super) async fn recognise(
        &self,
        metadata: &TitleSources,
        searched: &Searched<'_>,
    ) -> Result<Option<Recognised>, Miss> {
        let asked = &searched.asked();
        for plugin in &self.order {
            let Some(named) = self.ask(plugin, asked).await? else {
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

    /// The well-formed ids the Plugin `plugin`'s Recogniser names for `asked`, in its order;
    /// `None` when it is no longer enabled or doesn't serve `recognise_title`. Records whether
    /// the call failed for the Admin to see, and counts a failure for [`Self::log_failures`].
    async fn ask(
        &self,
        plugin: &PluginId,
        asked: &TitleToRecognise,
    ) -> Result<Option<Vec<ExternalId>>, Miss> {
        let answered = (self.plugins)
            .ask(plugin, |recogniser: Arc<dyn Recogniser>| async move {
                if !recogniser.recognises(RecogniserMethod::Title) {
                    return Ok(None);
                }
                recogniser.recognise_title(asked).await.map(Some)
            })
            .await;
        let point = PointKey::provide::<dyn Recogniser>();
        match answered {
            Some(Answer {
                result: Ok(Some(recognised)),
                ..
            }) => {
                self.plugins.provider_answered(plugin, point);
                Ok(Some(well_formed(plugin, &recognised)))
            }
            Some(Answer {
                result: Err(failure),
                ..
            }) => {
                self.plugins
                    .provider_failed(plugin, point, &failure.message);
                let mut failed = self.failures();
                let (titles, last) = failed.entry(plugin.clone()).or_default();
                *titles = titles.saturating_add(1);
                *last = failure.message;
                Err(Miss::RecogniserFailed)
            }
            // Doesn't serve `recognise_title`, isn't enabled or a Recogniser, or it panicked,
            // which disabled it.
            Some(Answer {
                result: Ok(None), ..
            })
            | None => Ok(None),
        }
    }
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
