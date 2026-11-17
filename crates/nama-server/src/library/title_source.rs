//! Asking the title sources for titles and images, and what their failures mean: no title source
//! enabled, or one that failed, is [`Unanswered::Unavailable`]; a title or image it doesn't have, an id
//! it can't look titles up by, or a method it doesn't serve, is [`Unanswered::NotFound`]. It alone asks
//! (see `src/library/AGENTS.md`).

use std::{future::Future, sync::Arc};

use nama_plugin_api::{
    PluginId,
    contract::{self as wire, ArtworkAsked, ImageAsked, SeasonAsked, TitleAsked},
    metadata::{ExternalId, ExternalIds, MetadataError, TitleMethod, TitleSource},
};

mod titles;

pub(crate) use titles::{
    Artwork, ArtworkKind, Candidate, Episode, Image, Movie, SearchQuery, Season, Show, TitleKind,
};

use crate::{
    library::contents::{ImageRef, Metadata},
    plugins::{Answer, ExtensionPoints, Failure, ProviderError},
};

/// Why the title source gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Unanswered {
    /// It has no such title, Season or image, or can't look titles up by the id asked for.
    #[error("the title source has no such title, Season or image")]
    NotFound,
    /// No title source is enabled, or it failed. Worth asking again later.
    #[error("no title source could answer")]
    Unavailable,
}

/// The title sources among [`ExtensionPoints`]: a Movie asked through the one that comes
/// first, or only through one Plugin's; everything else, through the routing methods, in turn by
/// namespace.
#[derive(Debug, Clone)]
pub(crate) struct TitleSources {
    plugins: ExtensionPoints,
    only: Option<PluginId>,
    /// The namespaces of the sources the routing methods ask, in the order they ask them; every
    /// source, in admin provider precedence, when `None`.
    order: Option<Vec<String>>,
}

/// An enabled title source, the namespace it keeps titles under, the others it looks them up by
/// and the methods it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    /// The Plugin serving it.
    pub(crate) plugin: PluginId,
    /// The namespace it keeps titles under.
    pub(crate) namespace: String,
    /// The well-formed namespaces besides `namespace` it looks titles up by.
    pub(crate) looks_up: Vec<String>,
    /// The methods it answers; the routing methods ask it nothing else.
    pub(crate) answers: Vec<TitleMethod>,
}

impl Source {
    /// Whether it searches for titles; one that doesn't only adds artwork and the ids it looks up.
    pub(crate) fn searches(&self) -> bool {
        self.answers.contains(&TitleMethod::Search)
    }
}

/// Every [`TitleMethod`], for reading which a source answers.
const METHODS: [TitleMethod; 7] = [
    TitleMethod::Search,
    TitleMethod::Movie,
    TitleMethod::Show,
    TitleMethod::Season,
    TitleMethod::Episode,
    TitleMethod::Image,
    TitleMethod::Artwork,
];

impl TitleSources {
    pub(crate) const fn new(plugins: ExtensionPoints) -> Self {
        Self {
            plugins,
            only: None,
            order: None,
        }
    }

    /// The same title sources, asked only through the Plugin `plugin`'s, which is
    /// [`Unanswered::Unavailable`] while it isn't enabled.
    pub(crate) fn only(&self, plugin: PluginId) -> Self {
        Self {
            plugins: self.plugins.clone(),
            only: Some(plugin),
            order: self.order.clone(),
        }
    }

    /// The metadata of the Movie with the id `id`.
    pub(crate) async fn movie(&self, id: &ExternalId) -> Result<Metadata, Unanswered> {
        let asked = &asked(id);
        let (plugin, movie) = self
            .ask(|metadata| async move {
                let found = metadata.movie(asked).await?;
                title(found.unsupported_id, found.movie, titles::movie)
            })
            .await?;
        Ok(Metadata::movie(plugin, movie))
    }

    /// The image at `path` from the Plugin `plugin`'s title source, which handed the path out.
    pub(crate) async fn image(&self, plugin: &PluginId, path: &str) -> Result<Image, Unanswered> {
        let asked = &ImageAsked {
            path: path.to_owned(),
            ..Default::default()
        };
        let answered = self
            .plugins
            .ask(plugin, |metadata: Arc<dyn TitleSource>| async move {
                let image = metadata.image(asked).await?.image;
                answer(image, |image| Ok(titles::image(image)))
            })
            .await;
        meaning(answered).map(|(_, image)| image)
    }

    /// The same sources, the routing methods asking only those whose namespace is in `order`, in
    /// that order, such as a Library's.
    pub(crate) fn in_order(&self, order: Vec<String>) -> Self {
        Self {
            plugins: self.plugins.clone(),
            only: self.only.clone(),
            order: Some(order),
        }
    }

    /// The enabled title sources the routing methods ask, in the order they ask them. A source
    /// whose namespace isn't well formed is left out, as is a malformed namespace it looks up by.
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
                    let namespace = source.namespace().to_owned();
                    let answers = METHODS.into_iter().filter(|&m| source.answers(m)).collect();
                    Ok::<_, MetadataError>((namespace, source.lookup_namespaces(), answers))
                })
                .await;
            let Ok((_, (namespace, mut looks_up, answers))) = meaning(answered) else {
                continue;
            };
            if !well_formed(&namespace) {
                tracing::warn!(%plugin, namespace, "the title source's namespace isn't well formed");
                continue;
            }
            looks_up.retain(|lookup| well_formed(lookup));
            sources.push(Source {
                plugin,
                namespace,
                looks_up,
                answers,
            });
        }
        if let Some(order) = &self.order {
            sources.retain(|source| order.contains(&source.namespace));
            sources.sort_by_key(|source| order.iter().position(|n| *n == source.namespace));
        }
        sources
    }

    /// Whether some source the routing methods ask answers `method` and keeps titles under
    /// `namespace`, such as an ID Hint's, or looks them up by it. Matching routes with
    /// [`TitleSources::by_namespace`] and keeps the route, so only tests ask this.
    #[cfg(test)]
    pub(crate) async fn serves(&self, namespace: &str, method: TitleMethod) -> bool {
        !self.by_namespace(namespace, method).await.is_empty()
    }

    /// Whether title sources are enabled but none answering `method` keeps or looks titles up by
    /// `id`'s namespace, so none could have the title; with none enabled, fetching answers
    /// [`Unanswered::Unavailable`] instead.
    pub(crate) async fn unserved(&self, id: &ExternalId, method: TitleMethod) -> bool {
        let sources = self.sources().await;
        !sources.is_empty() && routed(sources, id.namespace(), method).is_empty()
    }

    /// The titles the first source in order to find any finds for `query`, best fit first, with
    /// that source. A source that has none, or doesn't search, passes to the next; one that fails
    /// stops the search as [`Unanswered::Unavailable`], so a source being down never changes which
    /// source Matches.
    pub(crate) async fn search_in_turn(
        &self,
        query: &SearchQuery,
    ) -> Result<(Source, Vec<Candidate>), Unanswered> {
        let mut sources = self.sources().await;
        sources.retain(Source::searches);
        let query = &titles::search_query(query);
        self.first_answer(sources, Vec::is_empty, |metadata| async move {
            candidates(metadata.search(query).await?)
        })
        .await
    }

    /// The metadata of the Movie with the id `id`, from the source keeping titles under its
    /// namespace, or else the first one in order looking them up by it to have it, with that
    /// source. Fails as [`TitleSources::search_in_turn`] does, so with no such source it is
    /// [`Unanswered::Unavailable`].
    pub(crate) async fn movie_by(&self, id: &ExternalId) -> Result<(Source, Metadata), Unanswered> {
        let sources = self.by_namespace(id.namespace(), TitleMethod::Movie).await;
        self.movie_from(sources, id).await
    }

    /// The Movie with the id `id`, as [`TitleSources::movie_by`] fetches it, from `sources` already
    /// routed by [`TitleSources::by_namespace`].
    pub(crate) async fn movie_from(
        &self,
        sources: Vec<Source>,
        id: &ExternalId,
    ) -> Result<(Source, Metadata), Unanswered> {
        let asked = &asked(id);
        let (source, movie) = self
            .first_answer(
                sources,
                |_| false,
                |metadata| async move {
                    let found = metadata.movie(asked).await?;
                    title(found.unsupported_id, found.movie, titles::movie)
                },
            )
            .await?;
        let metadata = Metadata::movie(source.plugin.clone(), movie);
        Ok((source, metadata))
    }

    /// The Show with the id `id`, routed as [`TitleSources::movie_by`] routes a Movie.
    pub(crate) async fn show_by(&self, id: &ExternalId) -> Result<(Source, Show), Unanswered> {
        let sources = self.by_namespace(id.namespace(), TitleMethod::Show).await;
        self.show_from(sources, id).await
    }

    /// The Show with the id `id`, as [`TitleSources::show_by`] fetches it, from `sources` already
    /// routed by [`TitleSources::by_namespace`].
    pub(crate) async fn show_from(
        &self,
        sources: Vec<Source>,
        id: &ExternalId,
    ) -> Result<(Source, Show), Unanswered> {
        let asked = &asked(id);
        self.first_answer(
            sources,
            |_| false,
            |metadata| async move {
                let found = metadata.show(asked).await?;
                title(found.unsupported_id, found.show, titles::show)
            },
        )
        .await
    }

    /// The Season `number` of the Show with the id `show`, asked only of `source`, which keeps the
    /// Show.
    pub(crate) async fn season_from(
        &self,
        source: &Source,
        show: &ExternalId,
        number: u32,
    ) -> Result<Season, Unanswered> {
        let asked = &season_asked(show, number);
        let answered = self
            .plugins
            .ask(
                &source.plugin,
                |metadata: Arc<dyn TitleSource>| async move {
                    let found = metadata.season(asked).await?;
                    title(found.unsupported_id, found.season, titles::season)
                },
            )
            .await;
        meaning(answered).map(|(_, season)| season)
    }

    /// The artwork list of each source in order that answers [`TitleMethod::Artwork`] and has a
    /// list for the title of `kind` with the ids `ids`, asked by the title's id in the source's own
    /// namespace, or else in the first namespace it looks titles up by that the title has an id in.
    /// A source with no such id isn't asked; one that fails is logged and left out, so artwork
    /// never leaves a title undecided.
    pub(crate) async fn artwork_lists(
        &self,
        kind: TitleKind,
        ids: &ExternalIds,
    ) -> Vec<(Source, Vec<Artwork>)> {
        let mut lists = Vec::new();
        for source in self.sources().await {
            if !source.answers.contains(&TitleMethod::Artwork) {
                continue;
            }
            let id = (ids.get(&source.namespace)).or_else(|| {
                source
                    .looks_up
                    .iter()
                    .find_map(|namespace| ids.get(namespace))
            });
            let Some(id) = id else {
                continue;
            };
            let asked = &ArtworkAsked {
                kind: titles::title_kind(kind).into(),
                id: titles::title_id(&id).into(),
                ..Default::default()
            };
            let answered = self
                .plugins
                .ask(
                    &source.plugin,
                    |metadata: Arc<dyn TitleSource>| async move {
                        artwork(metadata.artwork(asked).await?)
                    },
                )
                .await;
            // `meaning` logs a failure, without the Plugin's secrets.
            if let Ok((_, list)) = meaning(answered)
                && !list.is_empty()
            {
                lists.push((source, list));
            }
        }
        lists
    }

    /// The sources answering `method` that keep titles under `namespace`, then those looking them
    /// up by it, each in order; never one that does neither.
    pub(crate) async fn by_namespace(&self, namespace: &str, method: TitleMethod) -> Vec<Source> {
        routed(self.sources().await, namespace, method)
    }

    /// The first answer of `sources`, asked in turn, that isn't `empty`. Not found, or an empty
    /// answer, passes to the next source; a failure stops. [`Unanswered::NotFound`] when every source
    /// passed, [`Unanswered::Unavailable`] when there was none.
    async fn first_answer<R, F>(
        &self,
        sources: Vec<Source>,
        empty: impl Fn(&R) -> bool,
        call: impl Fn(Arc<dyn TitleSource>) -> F,
    ) -> Result<(Source, R), Unanswered>
    where
        F: Future<Output = Result<R, Refusal>>,
    {
        if sources.is_empty() {
            tracing::debug!("no title source could be asked");
            return Err(Unanswered::Unavailable);
        }
        for source in sources {
            match meaning(self.plugins.ask(&source.plugin, &call).await) {
                Ok((_, answer)) if !empty(&answer) => return Ok((source, answer)),
                Ok(_) | Err(Unanswered::NotFound) => {}
                Err(Unanswered::Unavailable) => return Err(Unanswered::Unavailable),
            }
        }
        Err(Unanswered::NotFound)
    }

    async fn ask<R, F>(
        &self,
        call: impl Fn(Arc<dyn TitleSource>) -> F,
    ) -> Result<(PluginId, R), Unanswered>
    where
        F: Future<Output = Result<R, Refusal>>,
    {
        let answered = match &self.only {
            Some(plugin) => self.plugins.ask(plugin, call).await,
            None => self.plugins.ask_first(call).await,
        };
        meaning(answered)
    }
}

impl TitleKind {
    /// The method that fetches a title of this kind by its id.
    pub(crate) const fn method(self) -> TitleMethod {
        match self {
            Self::Movie => TitleMethod::Movie,
            Self::Show => TitleMethod::Show,
        }
    }
}

/// Those of `sources` [`TitleSources::by_namespace`] asks, in the order it asks them.
fn routed(mut sources: Vec<Source>, namespace: &str, method: TitleMethod) -> Vec<Source> {
    sources.retain(|source| {
        source.answers.contains(&method)
            && (source.namespace == namespace || source.looks_up.iter().any(|n| n == namespace))
    });
    sources.sort_by_key(|source| source.namespace != namespace);
    sources
}

/// The namespace check `ExternalId::new` makes, with a stand-in value.
fn well_formed(namespace: &str) -> bool {
    ExternalId::new(namespace, "_").is_ok()
}

fn asked(id: &ExternalId) -> TitleAsked {
    TitleAsked {
        id: titles::title_id(id).into(),
        ..Default::default()
    }
}

fn season_asked(show: &ExternalId, season: u32) -> SeasonAsked {
    SeasonAsked {
        show: titles::title_id(show).into(),
        season,
        ..Default::default()
    }
}

/// Why a source gave no title: it refused, answered that it can't look titles up by the asked
/// id's namespace, or answered with the title unset, which means it has no such title.
enum Refusal {
    Refused(MetadataError),
    UnsupportedId,
    Unset,
}

impl From<MetadataError> for Refusal {
    fn from(error: MetadataError) -> Self {
        Self::Refused(error)
    }
}

impl ProviderError<Unanswered> for Refusal {
    fn failure(self) -> Failure<Unanswered> {
        match self {
            Self::Refused(error) => error.failure(),
            Self::UnsupportedId => Failure {
                kind: Unanswered::NotFound,
                message: "the title source can't look titles up by that id".to_owned(),
            },
            Self::Unset => Failure {
                kind: Unanswered::NotFound,
                message: "the title source has no such title".to_owned(),
            },
        }
    }
}

/// The title in an answer's `field` as the Kernel's, through `kernel`; unset, it is
/// [`Refusal::Unset`], and malformed, a failure.
fn answer<W, T>(
    field: impl Into<Option<W>>,
    kernel: fn(W) -> Result<T, MetadataError>,
) -> Result<T, Refusal> {
    let title = field.into().ok_or(Refusal::Unset)?;
    Ok(kernel(title)?)
}

/// The title in an answer, as [`answer`] reads it, unless the source answered `unsupported_id`.
fn title<W, T>(
    unsupported_id: bool,
    field: impl Into<Option<W>>,
    kernel: fn(W) -> Result<T, MetadataError>,
) -> Result<T, Refusal> {
    if unsupported_id {
        return Err(Refusal::UnsupportedId);
    }
    answer(field, kernel)
}

/// The Kernel's twins of a search's Candidates.
fn candidates(found: wire::SearchResults) -> Result<Vec<Candidate>, Refusal> {
    let found = found.candidates.into_iter().map(titles::candidate);
    Ok(found.collect::<Result<_, _>>()?)
}

/// The Kernel's twin of an artwork list, unless the source answered `unsupported_id`.
fn artwork(found: wire::ArtworkAnswer) -> Result<Vec<Artwork>, Refusal> {
    if found.unsupported_id {
        return Err(Refusal::UnsupportedId);
    }
    Ok(titles::artwork(found.artwork))
}

impl ProviderError<Unanswered> for MetadataError {
    fn failure(self) -> Failure<Unanswered> {
        let kind = match self {
            Self::Unsupported => Unanswered::NotFound,
            Self::Failed(_) => Unanswered::Unavailable,
        };
        Failure {
            kind,
            message: self.to_string(),
        }
    }
}

/// The answer in `answered`, or what its failure means; `answered` is `None` when no title source
/// could be asked.
fn meaning<R>(answered: Option<Answer<R, Unanswered>>) -> Result<(PluginId, R), Unanswered> {
    match answered {
        Some(Answer {
            plugin,
            result: Ok(answer),
        }) => Ok((plugin, answer)),
        Some(Answer {
            plugin,
            result: Err(failure),
        }) => {
            if failure.kind == Unanswered::Unavailable {
                tracing::warn!(%plugin, error = failure.message, "the title source failed");
            }
            Err(failure.kind)
        }
        None => {
            tracing::debug!("no title source could be asked");
            Err(Unanswered::Unavailable)
        }
    }
}

impl Metadata {
    fn movie(plugin: PluginId, movie: Movie) -> Self {
        let poster = ImageRef::of(&plugin, movie.poster.as_ref());
        let backdrop = ImageRef::of(&plugin, movie.backdrop.as_ref());
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
            poster,
            backdrop,
            logo: None,
            still: None,
            genres: movie.genres,
            content_rating: movie.content_rating,
            show_poster: None,
            show_overview: None,
            show_first_air_date: None,
        }
    }
}

#[cfg(test)]
#[path = "title_source_tests.rs"]
mod tests;
