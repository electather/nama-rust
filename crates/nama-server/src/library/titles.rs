//! Finding a title of a Library by any of its External IDs (ADR 0042), for Browse and for deleting
//! media alike, and which Library has the titles some ids name, for Home rows and Similar titles.

use std::collections::{HashMap, HashSet};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    Contents, FoundMovie, FoundShow, Library, LibraryEpisode as Episode, LibraryKind as Kind,
    LibraryMovie as Movie, LibrarySeason as Season, LibraryShow as Show,
};

/// A title of a Library, which a User marks played or unplayed, or deletes.
#[derive(Debug, Clone)]
pub(crate) enum Target {
    Movie(ExternalId),
    Episode(ExternalId),
    Season { show: ExternalId, number: i64 },
    Show(ExternalId),
}

/// What finding a title, or a Library or Version, found nothing of.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NotFound {
    #[error("there is no Library {0}")]
    NoSuchLibrary(i64),
    #[error("the Library isn't a {} Library", match .wanted {
        Kind::Movies => "Movies",
        Kind::Shows => "Shows",
    })]
    WrongKind { wanted: Kind },
    /// No Movie, Show or Episode in the Library has the id asked for.
    #[error("nothing in the Library is Matched to that id")]
    NothingMatched,
    #[error("the Show has no such Season in the Library")]
    NoSuchSeason,
    #[error("the Season has no such Episode in the Library")]
    NoSuchEpisode,
    #[error("there is no such Version")]
    NoSuchVersion,
    /// The Box Set isn't stored, or the User can't see it.
    #[error("there is no such Box Set")]
    NoSuchBoxSet,
    /// No title in a Library has a Role of the Person.
    #[error("no title on the Server has a Role of that Person")]
    NoSuchPerson,
}

/// A Library's titles, read whole, without Tracks.
pub(crate) enum Titles {
    Movies(Vec<Movie>),
    Shows(Vec<Show>),
}

impl Titles {
    /// Every title of `library`, as `contents` has it.
    // ponytail: whole-Library read (see `crates/nama-server/AGENTS.md`).
    pub(crate) async fn read(contents: &Contents, library: &Library) -> Result<Self, sqlx::Error> {
        Ok(match library.kind {
            Kind::Movies => Self::Movies(contents.movies(library.id, None).await?),
            Kind::Shows => Self::Shows(contents.shows(library.id, None).await?),
        })
    }

    pub(crate) fn movies(self) -> Result<Vec<Movie>, NotFound> {
        match self {
            Self::Movies(movies) => Ok(movies),
            Self::Shows(_) => Err(NotFound::WrongKind {
                wanted: Kind::Movies,
            }),
        }
    }

    pub(crate) fn shows(self) -> Result<Vec<Show>, NotFound> {
        match self {
            Self::Shows(shows) => Ok(shows),
            Self::Movies(_) => Err(NotFound::WrongKind {
                wanted: Kind::Shows,
            }),
        }
    }

    /// The Movie with `id` among its ids: any one identifies it (ADR 0042).
    pub(crate) fn movie(self, id: &ExternalId) -> Result<Movie, NotFound> {
        (self.movies()?.into_iter())
            .find(|movie| known_as(&movie.ids, id))
            .ok_or(NotFound::NothingMatched)
    }

    /// The Show with `id` among its ids: any one identifies it (ADR 0042).
    pub(crate) fn show(self, id: &ExternalId) -> Result<Show, NotFound> {
        (self.shows()?.into_iter())
            .find(|show| known_as(&show.ids, id))
            .ok_or(NotFound::NothingMatched)
    }

    pub(crate) fn season(self, show: &ExternalId, number: i64) -> Result<Season, NotFound> {
        (self.show(show)?.seasons.remove(&number)).ok_or(NotFound::NoSuchSeason)
    }

    /// The Episode `id`, with the first Show that has it.
    pub(crate) fn episode(self, id: &ExternalId) -> Result<(Show, Episode), NotFound> {
        (self.shows()?.into_iter())
            .find_map(|show| {
                let episode = show.episodes().find(|episode| &episode.id == id)?.clone();
                Some((show, episode))
            })
            .ok_or(NotFound::NothingMatched)
    }
}

/// A title on the Server, in the oldest Library that has it.
#[derive(Debug, Clone)]
pub(crate) enum OnServer {
    Movie(FoundMovie),
    Show(FoundShow),
}

impl OnServer {
    /// The titles of `libraries`, oldest first, in its Libraries of `kind`, that have any of
    /// `ids`, each under every one of its ids among `ids` and in the oldest Library that has it.
    pub(crate) async fn find(
        contents: &Contents,
        libraries: &[Library],
        kind: Kind,
        ids: &[ExternalId],
    ) -> Result<HashMap<ExternalId, Self>, sqlx::Error> {
        let mut found = HashMap::new();
        if ids.is_empty() {
            return Ok(found);
        }
        for library in libraries.iter().filter(|library| library.kind == kind) {
            let titles: Vec<Self> = match kind {
                Kind::Movies => (contents.movies(library.id, Some(ids)).await?.iter())
                    .map(|movie| Self::movie(library.id, movie))
                    .collect(),
                Kind::Shows => (contents.shows(library.id, Some(ids)).await?.iter())
                    .map(|show| Self::show(library.id, show))
                    .collect(),
            };
            for title in titles {
                title.add_to(&mut found, ids);
            }
        }
        Ok(found)
    }

    /// As [`Self::find`], but in the order of `ids`, a title named by two of them once, at the
    /// first.
    pub(crate) async fn in_order_of(
        contents: &Contents,
        libraries: &[Library],
        kind: Kind,
        ids: &[ExternalId],
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut found = Self::find(contents, libraries, kind, ids).await?;
        let mut shown = HashSet::new();
        Ok((ids.iter())
            .filter_map(|id| found.remove(id))
            .filter(|title| shown.insert(title.id().clone()))
            .collect())
    }

    /// Adds it to `found` under each of its ids among `ids` that an older Library's title didn't
    /// take.
    fn add_to(&self, found: &mut HashMap<ExternalId, Self>, ids: &[ExternalId]) {
        for id in self.ids().iter().filter(|id| ids.contains(id)) {
            found.entry(id).or_insert_with(|| self.clone());
        }
    }

    fn movie(library: i64, movie: &Movie) -> Self {
        Self::Movie(FoundMovie {
            library,
            id: movie.id.clone(),
            ids: movie.ids.clone(),
            title: movie.title.clone(),
            release_date: movie.release_date.clone(),
            poster: movie.poster.clone(),
        })
    }

    fn show(library: i64, show: &Show) -> Self {
        Self::Show(FoundShow {
            library,
            id: show.id.clone(),
            ids: show.ids.clone(),
            title: show.title.clone(),
            first_air_date: show.first_air_date.clone(),
            poster: show.poster().cloned(),
        })
    }

    /// The id browsing finds it by in its Library.
    fn id(&self) -> &ExternalId {
        match self {
            Self::Movie(movie) => &movie.id,
            Self::Show(show) => &show.id,
        }
    }

    /// The Library it is in.
    pub(crate) const fn library(&self) -> i64 {
        match self {
            Self::Movie(movie) => movie.library,
            Self::Show(show) => show.library,
        }
    }

    /// When it was released, or first aired, as `YYYY-MM-DD`.
    pub(crate) fn date(&self) -> Option<&str> {
        match self {
            Self::Movie(movie) => movie.release_date.as_deref(),
            Self::Show(show) => show.first_air_date.as_deref(),
        }
    }

    pub(crate) fn ids(&self) -> &ExternalIds {
        match self {
            Self::Movie(movie) => &movie.ids,
            Self::Show(show) => &show.ids,
        }
    }
}

/// Whether `id` is one of `ids`: any one identifies a title (ADR 0042).
pub(crate) fn known_as(ids: &ExternalIds, id: &ExternalId) -> bool {
    ids.get(id.namespace()).as_ref() == Some(id)
}

/// The Episodes of `shows` that `target` names, with its name for the log and their Show. A Movie
/// is in no Shows Library.
pub(crate) fn named_episodes<'a>(
    shows: &'a [Show],
    target: &Target,
) -> Result<(String, &'a Show, Vec<&'a Episode>), NotFound> {
    let show_of = |id: &ExternalId| {
        (shows.iter().find(|show| known_as(&show.ids, id))).ok_or(NotFound::NothingMatched)
    };
    match target {
        Target::Movie(_) => Err(NotFound::WrongKind {
            wanted: Kind::Movies,
        }),
        Target::Show(id) => {
            let show = show_of(id)?;
            Ok((show.title.clone(), show, show.episodes().collect()))
        }
        Target::Season { show, number } => {
            let show = show_of(show)?;
            let season = show.seasons.get(number).ok_or(NotFound::NoSuchSeason)?;
            let name = format!("{} Season {number}", show.title);
            Ok((name, show, season.episodes.iter().collect()))
        }
        Target::Episode(id) => (shows.iter())
            .find_map(|show| {
                let episode = show.episodes().find(|episode| &episode.id == id)?;
                Some((episode_name(&show.title, episode), show, vec![episode]))
            })
            .ok_or(NotFound::NothingMatched),
    }
}

/// `episode` of the Show `show` as people name it, such as "Dark S01E02".
pub(crate) fn episode_name<V>(show: &str, episode: &Episode<V>) -> String {
    format!("{show} S{:02}E{:02}", episode.season, episode.number)
}

#[cfg(test)]
#[path = "titles_tests.rs"]
mod tests;
