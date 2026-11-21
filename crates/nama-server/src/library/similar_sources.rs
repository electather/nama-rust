//! Asking the Similar sources which titles are like a title, and merging their answers into one
//! list (ADR 0084). It alone asks them (see `src/library/AGENTS.md`).

use std::sync::Arc;

use futures_util::future::join_all;
use nama_plugin_api::{
    PluginId,
    contract::{SimilarAsked, SimilarTitles},
    metadata::{ExternalId, ExternalIds},
    similar_source::SimilarSource,
};

use super::title_source::{Roles, TitleKind, title_id, title_kind, title_roles};
use crate::plugins::ExtensionPoints;

/// The most titles a merged list shows: about a row of posters.
pub(crate) const MAX_SIMILAR: usize = 20;

/// The most titles the Kernel keeps of one answer, the heaviest: enough that a source's titles in
/// a Library survive its others, while one answer's row stays small.
const MAX_KEPT: usize = 100;

/// What one Similar source answered about a title: the titles like it, in its order, each with
/// how alike, from 0.0 to 1.0.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Answer {
    pub(crate) plugin: PluginId,
    pub(crate) titles: Vec<(ExternalId, f32)>,
}

/// The enabled Similar sources among [`ExtensionPoints`].
#[derive(Debug, Clone)]
pub(crate) struct SimilarSources {
    plugins: ExtensionPoints,
}

impl SimilarSources {
    pub(crate) const fn new(plugins: ExtensionPoints) -> Self {
        Self { plugins }
    }

    /// What each enabled Similar source answers about the title of `kind` known by `ids`, with
    /// its `genres` and `roles`, all asked at once. One that fails is logged and left out, so the
    /// answer it stored last stays.
    pub(crate) async fn ask(
        &self,
        kind: TitleKind,
        ids: &ExternalIds,
        genres: &[String],
        roles: &Roles,
    ) -> Vec<Answer> {
        if ids.is_empty() {
            return Vec::new();
        }
        let asked = &SimilarAsked {
            kind: title_kind(kind).into(),
            ids: ids.iter().map(|id| title_id(&id)).collect(),
            genres: genres.to_vec(),
            roles: title_roles(roles),
            ..SimilarAsked::default()
        };
        let sources = self.plugins.providers::<dyn SimilarSource>();
        let answers = sources.into_iter().map(|plugin| async move {
            let call = |source: Arc<dyn SimilarSource>| async move { source.similar(asked).await };
            match self.plugins.ask(&plugin, call).await?.result {
                Ok(titles) => Some(Answer {
                    plugin,
                    titles: kept(titles),
                }),
                Err(failure) => {
                    tracing::warn!(%plugin, error = failure.message, "the Similar source failed");
                    None
                }
            }
        });
        join_all(answers).await.into_iter().flatten().collect()
    }

    /// Those of `answers` from a Plugin that is an enabled Similar source now, so a disabled
    /// one's titles drop at once, whoever disabled it (ADR 0084).
    pub(crate) fn of_enabled(&self, mut answers: Vec<Answer>) -> Vec<Answer> {
        let enabled = self.plugins.providers::<dyn SimilarSource>();
        answers.retain(|answer| enabled.contains(&answer.plugin));
        answers
    }
}

/// Every title of `answers`, heaviest first, a tie going to the lower Plugin id, then to the
/// source's own order. A title answered twice comes up at each; whoever shows them keeps the
/// first, by any id the two share.
pub(crate) fn merged(answers: &[Answer]) -> Vec<ExternalId> {
    let mut weighed: Vec<(&ExternalId, f32, &PluginId)> = (answers.iter())
        .flat_map(|answer| {
            let weighed = answer.titles.iter();
            weighed.map(|(id, weight)| (id, *weight, &answer.plugin))
        })
        .collect();
    // Stable, so a source's own order breaks its ties.
    weighed.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.2.cmp(b.2)));
    weighed.into_iter().map(|(id, ..)| id.clone()).collect()
}

/// What the Kernel keeps of an answer: its [`MAX_KEPT`] heaviest titles with a well-formed id and
/// a weight, each weight clamped to between 0.0 and 1.0, ties in the source's order. A malformed
/// one is dropped rather than failing the answer.
fn kept(answer: SimilarTitles) -> Vec<(ExternalId, f32)> {
    let mut kept: Vec<(ExternalId, f32)> = (answer.titles.into_iter())
        .filter(|title| !title.weight.is_nan())
        .filter_map(|title| {
            let id = title.id.as_option()?;
            let id = ExternalId::new(&id.namespace, id.value.clone()).ok()?;
            // `abs`, as -0.0 clamps to itself and would sort below 0.0 by `total_cmp`.
            Some((id, title.weight.clamp(0.0, 1.0).abs()))
        })
        .collect();
    kept.sort_by(|a, b| b.1.total_cmp(&a.1));
    kept.truncate(MAX_KEPT);
    kept
}

#[cfg(test)]
#[path = "similar_sources_tests.rs"]
mod tests;
