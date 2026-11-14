//! Asking the title sources for titles and images, and what their failures mean: no title source
//! enabled, or one that failed, is [`Error::Unavailable`]; a title or image it doesn't have, an id
//! it can't look titles up by, or a method it doesn't serve, is [`Error::NotFound`]. It alone asks
//! (see the crate's `AGENTS.md`).

use std::{future::Future, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{
        Candidate, ExternalId, ExternalIds, Image, MetadataError, Movie, SearchQuery, Season, Show,
        TitleSource,
    },
};

use crate::{
    library::contents::Metadata,
    plugins::{Answer, ExtensionPoints, Failure, ProviderError},
};

/// Why the metadata Provider gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    /// It has no such title, Season or image, or can't look titles up by the id asked for.
    #[error("the metadata Provider has no such title, Season or image")]
    NotFound,
    /// No metadata Provider is enabled, or it failed. Worth asking again later.
    #[error("no metadata Provider could answer")]
    Unavailable,
}

/// The metadata Providers among [`ExtensionPoints`], asked through the one that comes first, or
/// only through one Plugin's; or, through the routing methods, in turn by namespace.
#[derive(Debug, Clone)]
pub(crate) struct MetadataSource {
    plugins: ExtensionPoints,
    only: Option<PluginId>,
    /// The namespaces of the sources the routing methods ask, in the order they ask them; every
    /// source, in admin provider precedence, when `None`.
    order: Option<Vec<String>>,
}

/// An enabled title source and the namespace it keeps titles under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    /// The Plugin serving it.
    pub(crate) plugin: PluginId,
    /// The namespace it keeps titles under.
    pub(crate) namespace: String,
}

impl MetadataSource {
    pub(crate) const fn new(plugins: ExtensionPoints) -> Self {
        Self {
            plugins,
            only: None,
            order: None,
        }
    }

    /// The same Providers, asked only through the Plugin `plugin`'s, which is
    /// [`Error::Unavailable`] while it isn't enabled.
    pub(crate) fn only(&self, plugin: PluginId) -> Self {
        Self {
            plugins: self.plugins.clone(),
            only: Some(plugin),
            order: self.order.clone(),
        }
    }

    /// The titles the Provider finds for `query`, best fit first, with the Plugin whose Provider
    /// found them, which their image paths are for.
    pub(crate) async fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<(PluginId, Vec<Candidate>), Error> {
        self.ask(|metadata| {
            let query = query.clone();
            async move { metadata.search(&query).await }
        })
        .await
    }

    /// The metadata of the Movie with the id `id`.
    pub(crate) async fn movie(&self, id: &ExternalId) -> Result<Metadata, Error> {
        let (plugin, movie) = self
            .ask(|metadata| {
                let id = id.clone();
                async move { metadata.movie(&id).await }
            })
            .await?;
        Ok(Metadata::movie(plugin, movie))
    }

    /// The Show with the id `id`, and the Plugin whose Provider gave it.
    pub(crate) async fn show(&self, id: &ExternalId) -> Result<(PluginId, Show), Error> {
        self.ask(|metadata| {
            let id = id.clone();
            async move { metadata.show(&id).await }
        })
        .await
    }

    /// The Season `number` of the Show with the id `show`, with its Episodes.
    pub(crate) async fn season(&self, show: &ExternalId, number: u32) -> Result<Season, Error> {
        self.ask(|metadata| {
            let show = show.clone();
            async move { metadata.season(&show, number).await }
        })
        .await
        .map(|(_, season)| season)
    }

    /// The image at `path` from the Plugin `plugin`'s Provider, which handed the path out.
    pub(crate) async fn image(&self, plugin: &PluginId, path: &str) -> Result<Image, Error> {
        let answered = self
            .plugins
            .ask(plugin, |metadata: Arc<dyn TitleSource>| {
                let path = path.to_owned();
                async move { metadata.image(&path).await }
            })
            .await;
        meaning(answered).map(|(_, image)| image)
    }

    /// The same sources, the routing methods asking only those whose namespace is in `order`, in
    /// that order, such as a Library's.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "routing callers move over in later tasks")
    )]
    pub(crate) fn in_order(&self, order: Vec<String>) -> Self {
        Self {
            plugins: self.plugins.clone(),
            only: self.only.clone(),
            order: Some(order),
        }
    }

    /// The enabled title sources the routing methods ask, in the order they ask them. A source
    /// whose namespace isn't well formed is left out.
    // ponytail: asks each source its namespace per call; cache it if that shows up.
    pub(crate) async fn sources(&self) -> Vec<Source> {
        let mut sources = Vec::new();
        for plugin in self.plugins.ranked::<dyn TitleSource>() {
            if self.only.as_ref().is_some_and(|only| *only != plugin) {
                continue;
            }
            let answered = self
                .plugins
                .ask(&plugin, |source: Arc<dyn TitleSource>| async move {
                    Ok::<_, MetadataError>(source.namespace().to_owned())
                })
                .await;
            let Ok((_, namespace)) = meaning(answered) else {
                continue;
            };
            // The namespace check `ExternalId::new` makes, with a stand-in value.
            if ExternalId::new(&namespace, "_").is_err() {
                tracing::warn!(%plugin, namespace, "the title source's namespace isn't well formed");
                continue;
            }
            sources.push(Source { plugin, namespace });
        }
        if let Some(order) = &self.order {
            sources.retain(|source| order.contains(&source.namespace));
            sources.sort_by_key(|source| order.iter().position(|n| *n == source.namespace));
        }
        sources
    }

    /// Whether some source the routing methods ask keeps titles under `namespace`, such as an ID
    /// Hint's.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "routing callers move over in later tasks")
    )]
    pub(crate) async fn serves(&self, namespace: &str) -> bool {
        let sources = self.sources().await;
        sources.iter().any(|source| source.namespace == namespace)
    }

    /// The titles the first source in order to find any finds for `query`, best fit first, with
    /// that source. A source that has none, or doesn't search, passes to the next; one that fails
    /// stops the search as [`Error::Unavailable`], so a source being down never changes which
    /// source Matches.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "routing callers move over in later tasks")
    )]
    pub(crate) async fn search_in_turn(
        &self,
        query: &SearchQuery,
    ) -> Result<(Source, Vec<Candidate>), Error> {
        let sources = self.sources().await;
        self.first_answer(sources, Vec::is_empty, |metadata| {
            let query = query.clone();
            async move { metadata.search(&query).await }
        })
        .await
    }

    /// The metadata of the Movie with the id `id`, from the source keeping titles under its
    /// namespace, or else the first other one in order to have it, with that source. Fails as
    /// [`MetadataSource::search_in_turn`] does.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "routing callers move over in later tasks")
    )]
    pub(crate) async fn movie_by(&self, id: &ExternalId) -> Result<(Source, Metadata), Error> {
        let sources = self.by_namespace(id.namespace()).await;
        let (source, movie) = self
            .first_answer(
                sources,
                |_| false,
                |metadata| {
                    let id = id.clone();
                    async move { metadata.movie(&id).await }
                },
            )
            .await?;
        let metadata = Metadata::movie(source.plugin.clone(), movie);
        Ok((source, metadata))
    }

    /// The Show with the id `id`, routed as [`MetadataSource::movie_by`] routes a Movie.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "routing callers move over in later tasks")
    )]
    pub(crate) async fn show_by(&self, id: &ExternalId) -> Result<(Source, Show), Error> {
        let sources = self.by_namespace(id.namespace()).await;
        self.first_answer(
            sources,
            |_| false,
            |metadata| {
                let id = id.clone();
                async move { metadata.show(&id).await }
            },
        )
        .await
    }

    /// The Season `number` of the Show with the id `show`, asked only of `source`, which keeps the
    /// Show.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "routing callers move over in later tasks")
    )]
    pub(crate) async fn season_from(
        &self,
        source: &Source,
        show: &ExternalId,
        number: u32,
    ) -> Result<Season, Error> {
        let answered = self
            .plugins
            .ask(&source.plugin, |metadata: Arc<dyn TitleSource>| {
                let show = show.clone();
                async move { metadata.season(&show, number).await }
            })
            .await;
        meaning(answered).map(|(_, season)| season)
    }

    /// The sources in order, those keeping titles under `namespace` first.
    async fn by_namespace(&self, namespace: &str) -> Vec<Source> {
        let mut sources = self.sources().await;
        sources.sort_by_key(|source| source.namespace != namespace);
        sources
    }

    /// The first answer of `sources`, asked in turn, that isn't `empty`. Not found, or an empty
    /// answer, passes to the next source; a failure stops. [`Error::NotFound`] when every source
    /// passed, [`Error::Unavailable`] when there was none.
    async fn first_answer<R, F>(
        &self,
        sources: Vec<Source>,
        empty: impl Fn(&R) -> bool,
        call: impl Fn(Arc<dyn TitleSource>) -> F,
    ) -> Result<(Source, R), Error>
    where
        F: Future<Output = Result<R, MetadataError>>,
    {
        if sources.is_empty() {
            tracing::debug!("no title source could be asked");
            return Err(Error::Unavailable);
        }
        for source in sources {
            match meaning(self.plugins.ask(&source.plugin, &call).await) {
                Ok((_, answer)) if !empty(&answer) => return Ok((source, answer)),
                Ok(_) | Err(Error::NotFound) => {}
                Err(Error::Unavailable) => return Err(Error::Unavailable),
            }
        }
        Err(Error::NotFound)
    }

    async fn ask<R, F>(
        &self,
        call: impl Fn(Arc<dyn TitleSource>) -> F,
    ) -> Result<(PluginId, R), Error>
    where
        F: Future<Output = Result<R, MetadataError>>,
    {
        let answered = match &self.only {
            Some(plugin) => self.plugins.ask(plugin, call).await,
            None => self.plugins.ask_first(call).await,
        };
        meaning(answered)
    }
}

impl ProviderError<Error> for MetadataError {
    fn failure(self) -> Failure<Error> {
        let kind = match self {
            Self::NotFound | Self::Unsupported | Self::UnsupportedId(_) => Error::NotFound,
            Self::Failed(_) => Error::Unavailable,
        };
        Failure {
            kind,
            message: self.to_string(),
        }
    }
}

/// The answer in `answered`, or what its failure means; `answered` is `None` when no Provider
/// could be asked.
fn meaning<R>(answered: Option<Answer<R, Error>>) -> Result<(PluginId, R), Error> {
    match answered {
        Some(Answer {
            plugin,
            result: Ok(answer),
        }) => Ok((plugin, answer)),
        Some(Answer {
            plugin,
            result: Err(failure),
        }) => {
            if failure.kind == Error::Unavailable {
                tracing::warn!(%plugin, error = failure.message, "the metadata provider failed");
            }
            Err(failure.kind)
        }
        None => {
            tracing::debug!("no metadata provider could be asked");
            Err(Error::Unavailable)
        }
    }
}

impl Metadata {
    fn movie(plugin: PluginId, movie: Movie) -> Self {
        Self {
            plugin,
            ids: movie.ids,
            show_ids: ExternalIds::default(),
            title: movie.title,
            original_title: movie.original_title,
            show_title: None,
            overview: movie.overview,
            release_date: movie.release_date,
            runtime_minutes: movie.runtime_minutes,
            poster: movie.poster,
            backdrop: movie.backdrop,
            still: None,
        }
    }
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
