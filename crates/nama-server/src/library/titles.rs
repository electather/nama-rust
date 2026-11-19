//! Finding a title of a Library by any of its External IDs (ADR 0042), for Browse and for deleting
//! media alike.

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    Contents, Library, LibraryEpisode as Episode, LibraryKind as Kind, LibraryMovie as Movie,
    LibrarySeason as Season, LibraryShow as Show,
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
