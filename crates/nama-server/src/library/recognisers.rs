//! Asking the Recognisers a Library lists, at either of ADR 0050's levels: which are enabled and
//! in what order, whether one serves the level's method, asking it, and logging each one that
//! failed once per scan. What a proposal means stays with the level asking. It alone asks (see
//! `src/library/AGENTS.md`).

use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex, PoisonError},
};

use nama_plugin_api::{
    PluginError, PluginId,
    recogniser::{Recogniser, RecogniserMethod},
};

use crate::{
    library::{Libraries, Library},
    plugins::ExtensionPoints,
};

/// The enabled Recognisers a Library lists, asked at one level.
#[derive(Debug, Clone)]
pub(crate) struct Recognisers {
    plugins: ExtensionPoints,
    /// The method asked: [`RecogniserMethod::File`] for a scan, [`RecogniserMethod::Title`] for
    /// Matching.
    level: RecogniserMethod,
    /// First asked first: none until [`Recognisers::in_order`] lists some.
    listed: Vec<PluginId>,
    failures: Arc<Failures>,
}

/// A Recogniser failed, and is counted for its scan's log. Recorded for the Plugins page too.
#[derive(Debug)]
pub(crate) struct RecogniserFailed;

impl Recognisers {
    /// Recognisers asked at `level`, listing none until [`Recognisers::in_order`].
    pub(crate) fn new(plugins: ExtensionPoints, level: RecogniserMethod) -> Self {
        Self {
            plugins,
            level,
            listed: Vec::new(),
            failures: Arc::new(Failures::new(level)),
        }
    }

    /// Those of the Plugins `order` names that are enabled Recognisers now, in its order, as a
    /// Library's `recognisers` lists them, with no failures yet. Each failure they count is logged
    /// once, when the last clone of what this returns is dropped, however the scan ends.
    pub(crate) fn in_order(&self, order: &[String]) -> Self {
        let enabled = self.plugins.providers::<dyn Recogniser>();
        Self {
            listed: (order.iter().map(|id| PluginId::from(id.clone())))
                .filter(|id| enabled.contains(id))
                .collect(),
            failures: Arc::new(Failures::new(self.level)),
            ..self.clone()
        }
    }

    /// The Recognisers to ask, first asked first.
    pub(crate) fn listed(&self) -> &[PluginId] {
        &self.listed
    }

    /// What the Plugin `plugin`'s Recogniser answers to `call`, which calls this level's method;
    /// `None` when it doesn't serve the method or is no longer enabled.
    ///
    /// # Errors
    ///
    /// [`RecogniserFailed`] when it fails, counted for the log.
    pub(crate) async fn ask<R, F>(
        &self,
        plugin: &PluginId,
        call: impl Fn(Arc<dyn Recogniser>) -> F,
    ) -> Result<Option<R>, RecogniserFailed>
    where
        F: Future<Output = Result<R, PluginError>>,
    {
        let level = self.level;
        let serves = (self.plugins)
            .read_provider(plugin, |recogniser: &dyn Recogniser| {
                recogniser.recognises(level)
            })
            .await;
        if serves != Some(true) {
            return Ok(None);
        }
        // `None`: disabled since the scan began, or it panicked, which disabled it.
        let Some(answer) = self.plugins.ask(plugin, call).await else {
            return Ok(None);
        };
        answer.result.map(Some).map_err(|failure| {
            self.failures.add(answer.plugin, failure.message);
            RecogniserFailed
        })
    }

    /// Has Contents read `library`'s unparsed files again and forget what Matching left
    /// Unmatched there if the Recognisers it lists that are enabled, in its order, aren't those
    /// its last scan started with, and records them, in one write (ADR 0050). A scan calls it
    /// under the Library's scan lock before it reads what is stored, so the files it marks are
    /// read in that scan.
    pub(crate) async fn read_again_if_changed(
        &self,
        libraries: &Libraries,
        library: &Library,
    ) -> Result<(), sqlx::Error> {
        let listed = self.in_order(&library.recognisers).listed;
        let read: Vec<&str> = listed.iter().map(PluginId::as_str).collect();
        libraries
            .read_again_if_recognisers_changed(library.id, read.join(","))
            .await
    }
}

/// Each Recogniser that failed during one scan at `level`: how many files or titles it failed on,
/// and its last error. Logged once, when the scan lets go of it.
#[derive(Debug)]
struct Failures {
    level: RecogniserMethod,
    failed: Mutex<BTreeMap<PluginId, (usize, String)>>,
}

impl Failures {
    const fn new(level: RecogniserMethod) -> Self {
        Self {
            level,
            failed: Mutex::new(BTreeMap::new()),
        }
    }

    fn add(&self, plugin: PluginId, message: String) {
        let mut failed = self.failed.lock().unwrap_or_else(PoisonError::into_inner);
        let (count, last) = failed.entry(plugin).or_default();
        *count = count.saturating_add(1);
        *last = message;
    }
}

impl Drop for Failures {
    fn drop(&mut self) {
        let failed = self
            .failed
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        for (plugin, (count, last)) in failed.iter() {
            // The words say what each level does with what it failed on.
            match self.level {
                RecogniserMethod::File => tracing::warn!(
                    %plugin,
                    files = count,
                    last_error = last,
                    "a Recogniser failed, so the files it failed on are read again on the next scan"
                ),
                RecogniserMethod::Title => tracing::warn!(
                    %plugin,
                    titles = count,
                    last_error = last,
                    "a Recogniser failed, so the titles it failed on are matched again on the next scan"
                ),
            }
        }
    }
}

#[cfg(test)]
#[path = "recognisers_tests.rs"]
mod tests;
