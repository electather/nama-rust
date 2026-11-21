//! A title source a unit test scripts, so it states only the titles, search results, Seasons,
//! artwork and failures it needs, and reads back what was asked. A test that wants TMDB's own
//! behaviour enables the real TMDB Plugin on its HTTP fake instead.

use std::{
    future,
    sync::{Arc, Mutex, PoisonError},
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        Artwork, ArtworkAnswer, ArtworkAsked, ArtworkKind, Candidate, Episode, Movie, MovieAnswer,
        SearchQuery, SearchResults, Season, SeasonAnswer, SeasonAsked, Show, ShowAnswer,
        TitleAsked, TitleId,
    },
    metadata::{MetadataError, TitleMethod, TitleSource},
};

/// What the [`ScriptedSource`]s sharing it were asked, in the order asked.
#[derive(Debug, Clone, Default)]
pub(crate) struct SourcesAsked(Arc<Mutex<Vec<String>>>);

/// How the log notes a source being asked its namespace, after the namespace.
const NAMESPACE: &str = " namespace";

impl SourcesAsked {
    /// Each ask but for a namespace, as `<namespace> <method> <id>`, a Season's with its number
    /// after.
    pub(crate) fn lines(&self) -> Vec<String> {
        let mut lines = self.all();
        lines.retain(|line| !line.ends_with(NAMESPACE));
        lines
    }

    /// The namespaces of the sources asked their namespace, once per ask.
    pub(crate) fn namespaces(&self) -> Vec<String> {
        let lines = self.all().into_iter();
        lines
            .filter_map(|line| line.strip_suffix(NAMESPACE).map(str::to_owned))
            .collect()
    }

    fn all(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn note(&self, line: String) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line);
    }
}

/// How a [`ScriptedSource`] refuses a method instead of answering.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Refusal {
    /// It fails, as a source that can't be reached.
    Fails,
    /// It says it doesn't serve the method, though it lists it.
    Unsupported,
    /// It says it can't look titles up by the id asked.
    UnsupportedId,
}

/// A title source keeping titles under its namespace, served by the Plugin of that id at the
/// default priority, so sources rank by their namespaces. It has only what it's told, finds a
/// title by any of its ids, and serves every method unless told which.
#[derive(Debug, Clone)]
pub(crate) struct ScriptedSource {
    namespace: &'static str,
    looks_up: Vec<String>,
    found: Vec<Candidate>,
    movies: Vec<Movie>,
    poster: Option<String>,
    shows: Vec<Show>,
    /// Each Season with the id of its Show.
    seasons: Vec<(String, Season)>,
    artwork: Vec<Artwork>,
    serving: Option<Vec<TitleMethod>>,
    refusals: Vec<(TitleMethod, Refusal)>,
    asked: SourcesAsked,
}

impl ScriptedSource {
    /// A source under `namespace` that has nothing yet, noting what it's asked in `asked`.
    pub(crate) fn new(namespace: &'static str, asked: &SourcesAsked) -> Self {
        Self {
            namespace,
            looks_up: Vec::new(),
            found: Vec::new(),
            movies: Vec::new(),
            poster: None,
            shows: Vec::new(),
            seasons: Vec::new(),
            artwork: Vec::new(),
            serving: None,
            refusals: Vec::new(),
            asked: asked.clone(),
        }
    }

    /// Looks titles up by `namespaces` too.
    pub(crate) fn looking_up(mut self, namespaces: &[&str]) -> Self {
        self.looks_up = namespaces.iter().map(|&n| n.to_owned()).collect();
        self
    }

    /// Finds the title `title` with the ids `ids`, `ids[0]` its own, in every search, after those
    /// it finds already. Each has 100 votes, so none outvotes another.
    pub(crate) fn found(mut self, ids: &[&str], title: &str) -> Self {
        self.found.push(Candidate {
            id: title_id(ids[0]).into(),
            ids: title_ids(ids),
            title: title.to_owned(),
            vote_count: 100,
            ..Default::default()
        });
        self
    }

    /// Has the Movie `title` with the ids `ids`.
    pub(crate) fn with_movie(mut self, ids: &[&str], title: &str) -> Self {
        self.movies.push(Movie {
            ids: title_ids(ids),
            title: title.to_owned(),
            ..Default::default()
        });
        self
    }

    /// Gives each of its Movies the poster at `path`.
    pub(crate) fn poster(mut self, path: &str) -> Self {
        self.poster = Some(path.to_owned());
        self
    }

    /// Has the Show `title` with the ids `ids`, listing the Seasons `seasons`.
    pub(crate) fn with_show(mut self, ids: &[&str], title: &str, seasons: &[u32]) -> Self {
        self.shows.push(Show {
            ids: title_ids(ids),
            title: title.to_owned(),
            seasons: seasons.to_vec(),
            ..Default::default()
        });
        self
    }

    /// Has the Season `number` of the Show `show`, holding the Episodes `episodes`.
    pub(crate) fn with_season(mut self, show: &str, number: u32, episodes: &[u32]) -> Self {
        let episodes = (episodes.iter())
            .map(|&episode| Episode {
                season: number,
                number: episode,
                ..Default::default()
            })
            .collect();
        let season = Season {
            number,
            episodes,
            ..Default::default()
        };
        self.seasons.push((show.to_owned(), season));
        self
    }

    /// Gives the Episode `number` of the Season `season` of the Show `show` the id `id`, its own.
    pub(crate) fn episode_id(mut self, show: &str, season: u32, number: u32, id: &str) -> Self {
        let episode = (self.seasons.iter_mut())
            .filter(|(of, held)| of == show && held.number == season)
            .flat_map(|(_, held)| held.episodes.iter_mut())
            .find(|episode| episode.number == number)
            .expect("the Season holds the Episode");
        episode.ids = vec![title_id(id)];
        self
    }

    /// Lists the image of `kind` at `path` in `language` as the artwork of every title and Season
    /// it's asked about, after those it lists already.
    pub(crate) fn with_artwork(mut self, kind: ArtworkKind, path: &str, language: &str) -> Self {
        self.artwork.push(Artwork {
            kind: kind.into(),
            path: path.to_owned(),
            language: Some(language.to_owned()),
            ..Default::default()
        });
        self
    }

    /// Serves only `methods`, so the Server never asks it the others.
    pub(crate) fn serving(mut self, methods: &[TitleMethod]) -> Self {
        self.serving = Some(methods.to_vec());
        self
    }

    /// Answers `methods` with `refusal`.
    pub(crate) fn refusing(mut self, methods: &[TitleMethod], refusal: Refusal) -> Self {
        self.refusals
            .extend(methods.iter().map(|&method| (method, refusal)));
        self
    }

    /// Notes the ask `ask` of `method`, then answers it with what `found` makes, or its refusal.
    fn answer<'a, T: Send + 'a>(
        &self,
        method: TitleMethod,
        ask: &str,
        found: impl FnOnce(bool) -> T,
    ) -> BoxFuture<'a, Result<T, MetadataError>> {
        self.asked.note(format!("{} {ask}", self.namespace));
        let refusal =
            (self.refusals.iter()).find_map(|&(m, refusal)| (m == method).then_some(refusal));
        let answer = match refusal {
            Some(Refusal::Fails) => Err(MetadataError::Failed("unreachable".into())),
            Some(Refusal::Unsupported) => Err(MetadataError::Unsupported),
            Some(Refusal::UnsupportedId) => Ok(found(true)),
            None => Ok(found(false)),
        };
        Box::pin(future::ready(answer))
    }

    fn movie_by(&self, id: &TitleId) -> Option<Movie> {
        let movie = self.movies.iter().find(|movie| has_id(&movie.ids, id))?;
        Some(Movie {
            poster: self.poster.clone(),
            ..movie.clone()
        })
    }

    fn season_of(&self, show: &TitleId, number: u32) -> Option<Season> {
        let show = named(show);
        (self.seasons.iter())
            .find(|(of, season)| *of == show && season.number == number)
            .map(|(_, season)| season.clone())
    }
}

impl TitleSource for ScriptedSource {
    fn namespace(&self) -> &str {
        self.asked.note(format!("{}{NAMESPACE}", self.namespace));
        self.namespace
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        self.looks_up.clone()
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        self.answer(TitleMethod::Search, "search", |_| SearchResults {
            candidates: self.found.clone(),
            ..Default::default()
        })
    }

    fn movie<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        let ask = format!("movie {}", named(&asked.id));
        self.answer(TitleMethod::Movie, &ask, |unsupported_id| MovieAnswer {
            movie: self.movie_by(&asked.id).into(),
            unsupported_id,
            ..Default::default()
        })
    }

    fn show<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        let ask = format!("show {}", named(&asked.id));
        let show = self.shows.iter().find(|show| has_id(&show.ids, &asked.id));
        self.answer(TitleMethod::Show, &ask, |unsupported_id| ShowAnswer {
            show: show.cloned().into(),
            unsupported_id,
            ..Default::default()
        })
    }

    fn season<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        let ask = format!("season {} {}", named(&asked.show), asked.season);
        self.answer(TitleMethod::Season, &ask, |unsupported_id| SeasonAnswer {
            season: self.season_of(&asked.show, asked.season).into(),
            unsupported_id,
            ..Default::default()
        })
    }

    fn artwork<'a>(
        &'a self,
        asked: &'a ArtworkAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        let ask = format!("artwork {}", named(&asked.id));
        self.answer(TitleMethod::Artwork, &ask, |unsupported_id| ArtworkAnswer {
            artwork: self.artwork.clone(),
            unsupported_id,
            ..Default::default()
        })
    }

    fn season_artwork<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        let ask = format!("season_artwork {} {}", named(&asked.show), asked.season);
        self.answer(TitleMethod::SeasonArtwork, &ask, |unsupported_id| {
            ArtworkAnswer {
                artwork: self.artwork.clone(),
                unsupported_id,
                ..Default::default()
            }
        })
    }

    fn answers(&self, method: TitleMethod) -> bool {
        (self.serving.as_ref()).is_none_or(|serving| serving.contains(&method))
    }
}

impl Plugin for ScriptedSource {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static(self.namespace))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

/// The id `id` names as `<namespace>-<value>`, as an `ExternalId` shows.
fn title_id(id: &str) -> TitleId {
    let (namespace, value) = id.split_once('-').expect("an id is <namespace>-<value>");
    TitleId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

fn title_ids(ids: &[&str]) -> Vec<TitleId> {
    ids.iter().map(|&id| title_id(id)).collect()
}

/// `id` as `<namespace>-<value>`.
fn named(id: &TitleId) -> String {
    format!("{}-{}", id.namespace, id.value)
}

fn has_id(ids: &[TitleId], id: &TitleId) -> bool {
    ids.iter().any(|known| named(known) == named(id))
}
