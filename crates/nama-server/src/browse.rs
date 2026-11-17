//! Browse: the Movies and Shows a Library holds, as a User sees them.
//!
//! [`Browse`] finds titles through [`Contents`], never in the content tables itself (see
//! `src/library/AGENTS.md`), and attaches the User's [`WatchState`]s and, only to the Movies and
//! Episodes it answers with, their Versions' Tracks.
//! It writes only Watch State, for what a reported Version or a marked title stands for (ADR 0013).

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::{
    db::Database,
    library::{
        Contents, Found, Libraries, Library, LibraryEpisode, LibraryError, LibraryKind,
        LibraryMovie, LibrarySeason, LibraryShow, LibraryVersion, Stored, WatchState,
        WatchStateKind, WatchStates,
    },
    page,
    playback::{Probe, Track, Tracks},
    scanner::read_only_folder,
};

mod doomed;
mod watching;

pub(crate) use doomed::ToDelete;
use doomed::names;
pub(crate) use watching::Started;

/// A Movie whose Versions have their Tracks.
pub(crate) type Movie = LibraryMovie<Version>;
/// A Show, its Episodes' Versions without Tracks: nothing shown of a Show reads them.
pub(crate) type Show = LibraryShow;
/// A Season whose Episodes' Versions have their Tracks.
pub(crate) type Season = LibrarySeason<Version>;
/// An Episode whose Versions have their Tracks.
pub(crate) type Episode = LibraryEpisode<Version>;

/// A stored file that a Movie or Episode is available in, with its Tracks.
#[derive(Debug, Clone)]
pub(crate) struct Version {
    /// Its file as stored now: Tracks count only if probing read it at this size and time.
    pub(crate) file: Stored,
    pub(crate) path: String,
    /// What probing read from the file as it is now: none if it hasn't.
    pub(crate) tracks: Vec<Track>,
}

/// Why Browse found nothing, or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("there is no Library {0}")]
    NoSuchLibrary(i64),
    #[error("the Library isn't a {} Library", match .wanted {
        LibraryKind::Movies => "Movies",
        LibraryKind::Shows => "Shows",
    })]
    WrongKind { wanted: LibraryKind },
    /// No Movie, Show or Episode in the Library has the id asked for.
    #[error("nothing in the Library is Matched to that id")]
    NothingMatched,
    #[error("the Show has no such Season in the Library")]
    NoSuchSeason,
    #[error("there is no such Version")]
    NoSuchVersion,
    #[error(transparent)]
    PageTokenInvalid(#[from] page::TokenInvalid),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// A title of a Library, which a User marks played or unplayed ([`Browse::mark`]) or deletes
/// ([`ToDelete::Title`]).
#[derive(Debug)]
pub(crate) enum Target {
    Movie(ExternalId),
    Episode(ExternalId),
    Season { show: ExternalId, number: i64 },
    Show(ExternalId),
}

/// A Library's titles, read whole, without Tracks.
enum Titles {
    Movies(Vec<LibraryMovie>),
    Shows(Vec<LibraryShow>),
}

/// Reads the Libraries' titles and the Users' Watch States of them, and records Watch State.
#[derive(Debug, Clone)]
pub(crate) struct Browse {
    libraries: Libraries,
    contents: Contents,
    tracks: Tracks,
    watch_states: WatchStates,
}

impl Browse {
    pub(crate) fn new(db: Arc<Database>) -> Self {
        Self {
            libraries: Libraries::new(Arc::clone(&db)),
            contents: Contents::new(Arc::clone(&db)),
            tracks: Tracks::new(Arc::clone(&db)),
            watch_states: WatchStates::new(db),
        }
    }

    /// The Movies of the Movies Library `library`, each with the User `user`'s Watch State of
    /// it, if they've started it.
    pub(crate) async fn movies(
        &self,
        user: i64,
        library: i64,
    ) -> Result<Vec<(Movie, Option<WatchState>)>, Error> {
        let movies = self.titles(&self.library(library).await?).await?.movies()?;
        let probes = self
            .probes(movies.iter().flat_map(|movie| &movie.versions))
            .await?;
        let titles = movies.iter().map(movie_ids).collect();
        let states = self.states_of(user, titles).await?;
        let movies = movies
            .into_iter()
            .map(|movie| movie.map_versions(with_tracks(&probes)));
        Ok(movies.zip(states).collect())
    }

    /// The Movie `id` of the Movies Library `library`, with the User `user`'s Watch State of it,
    /// if they've started it, and the Library's folder holding one of its Versions that refuses
    /// deleting it, if one does.
    pub(crate) async fn movie(
        &self,
        user: i64,
        library: i64,
        id: &ExternalId,
    ) -> Result<(Movie, Option<WatchState>, Option<String>), Error> {
        let library = self.library(library).await?;
        let movie = self.titles(&library).await?.movie(id)?;
        let probes = self.probes(movie.versions.iter()).await?;
        let movie = movie.map_versions(with_tracks(&probes));
        let paths = movie.versions.iter().map(|version| version.path.as_str());
        let read_only_folder = read_only_folder(&library, paths).map(str::to_owned);
        let state = self.states_of(user, vec![movie_ids(&movie)]).await?.pop();
        Ok((movie, state.flatten(), read_only_folder))
    }

    /// The Episodes of Season `season` of the Show `show` of the Shows Library `library`, each
    /// with the User `user`'s Watch State of it, if they've started it.
    pub(crate) async fn episodes(
        &self,
        user: i64,
        library: i64,
        show: &ExternalId,
        season: i64,
    ) -> Result<Vec<(Episode, Option<WatchState>)>, Error> {
        let episodes = self.season(library, show, season).await?.episodes;
        let titles = episodes.iter().map(episode_ids).collect();
        let states = self.states_of(user, titles).await?;
        Ok(episodes.into_iter().zip(states).collect())
    }

    /// The Shows of the Shows Library `library`.
    pub(crate) async fn shows(&self, library: i64) -> Result<Vec<Show>, Error> {
        self.titles(&self.library(library).await?).await?.shows()
    }

    /// The Show `id` of the Shows Library `library`.
    pub(crate) async fn show(&self, library: i64, id: &ExternalId) -> Result<Show, Error> {
        self.titles(&self.library(library).await?).await?.show(id)
    }

    /// Season `number` of the Show `show` of the Shows Library `library`.
    pub(crate) async fn season(
        &self,
        library: i64,
        show: &ExternalId,
        number: i64,
    ) -> Result<Season, Error> {
        let library = self.library(library).await?;
        let season = self.titles(&library).await?.season(show, number)?;
        let files = season.episodes.iter().flat_map(|episode| &episode.versions);
        let probes = self.probes(files).await?;
        let episodes = (season.episodes.into_iter())
            .map(|episode| episode.map_versions(with_tracks(&probes)))
            .collect();
        Ok(LibrarySeason {
            poster: season.poster,
            episodes,
        })
    }

    /// The Movies, Shows and Episodes of every Library that a search for `query` finds: see
    /// [`Contents::search`].
    pub(crate) async fn search(&self, query: &str) -> Result<Found, Error> {
        Ok(self.contents.search(query).await?)
    }

    /// The User `user`'s Watch State of each of `titles`, in the same order, kept under any of
    /// its ids (ADR 0042): `None` for one they haven't started.
    async fn states_of(
        &self,
        user: i64,
        titles: Vec<(WatchStateKind, ExternalIds)>,
    ) -> Result<Vec<Option<WatchState>>, sqlx::Error> {
        let states = self.watch_states.of_titles(Some(user), &titles).await?;
        Ok((states.into_iter())
            .map(|of_title| of_title.first().map(|&(_, state)| state))
            .collect())
    }

    /// Every title of `library`, as [`Contents`] has it, without Tracks. Every finder reads
    /// through here; one answering with Tracks reads them afterwards, of only what it answers with.
    // ponytail: whole-Library read (see AGENTS.md).
    async fn titles(&self, library: &Library) -> Result<Titles, Error> {
        Ok(match library.kind {
            LibraryKind::Movies => Titles::Movies(self.contents.movies(library.id).await?),
            LibraryKind::Shows => Titles::Shows(self.contents.shows(library.id).await?),
        })
    }

    /// What probing read from the files of `versions` as they are now, by file id.
    async fn probes(
        &self,
        versions: impl Iterator<Item = &LibraryVersion>,
    ) -> Result<HashMap<i64, Probe>, sqlx::Error> {
        let files: Vec<Stored> = versions.map(|version| version.file).collect();
        if files.is_empty() {
            return Ok(HashMap::new());
        }
        self.tracks.of(&files).await
    }

    async fn library(&self, library: i64) -> Result<Library, Error> {
        self.libraries.get(library).await.map_err(finding_failed)
    }
}

impl Titles {
    fn movies(self) -> Result<Vec<LibraryMovie>, Error> {
        match self {
            Self::Movies(movies) => Ok(movies),
            Self::Shows(_) => Err(Error::WrongKind {
                wanted: LibraryKind::Movies,
            }),
        }
    }

    fn shows(self) -> Result<Vec<LibraryShow>, Error> {
        match self {
            Self::Shows(shows) => Ok(shows),
            Self::Movies(_) => Err(Error::WrongKind {
                wanted: LibraryKind::Shows,
            }),
        }
    }

    /// The Movie with `id` among its ids: any one identifies it (ADR 0042).
    fn movie(self, id: &ExternalId) -> Result<LibraryMovie, Error> {
        (self.movies()?.into_iter())
            .find(|movie| names(&movie.ids, id))
            .ok_or(Error::NothingMatched)
    }

    /// The Show with `id` among its ids: any one identifies it (ADR 0042).
    fn show(self, id: &ExternalId) -> Result<LibraryShow, Error> {
        (self.shows()?.into_iter())
            .find(|show| names(&show.ids, id))
            .ok_or(Error::NothingMatched)
    }

    fn season(self, show: &ExternalId, number: i64) -> Result<LibrarySeason, Error> {
        (self.show(show)?.seasons.remove(&number)).ok_or(Error::NoSuchSeason)
    }

    /// The Episode `id` of any of its Shows.
    fn episode(self, id: &ExternalId) -> Result<LibraryEpisode, Error> {
        (self.shows()?.into_iter())
            .flat_map(|show| show.seasons.into_values())
            .flat_map(|season| season.episodes)
            .find(|episode| &episode.id == id)
            .ok_or(Error::NothingMatched)
    }
}

/// A Movie as Watch State reads it: by any of its ids.
fn movie_ids<V>(movie: &LibraryMovie<V>) -> (WatchStateKind, ExternalIds) {
    (WatchStateKind::Movie, movie.ids.clone())
}

/// An Episode as Watch State reads it: by any of its ids.
fn episode_ids<V>(episode: &LibraryEpisode<V>) -> (WatchStateKind, ExternalIds) {
    (WatchStateKind::Episode, episode.ids.clone())
}

/// Makes a Version from Contents into one with the Tracks `probes` has of its file.
fn with_tracks(probes: &HashMap<i64, Probe>) -> impl Fn(LibraryVersion) -> Version {
    |version| {
        // A file holding two Episodes is a Version of each, so its Tracks are cloned.
        let tracks = (probes.get(&version.file.id)).map_or_default(|probe| probe.tracks.clone());
        Version {
            file: version.file,
            path: version.path,
            tracks,
        }
    }
}

/// What [`Libraries::get`] or [`Libraries::list`] failing means to Browse.
#[expect(
    clippy::unreachable,
    reason = "`Libraries::get` and `Libraries::list` only find Libraries; they never refuse a \
              change"
)]
fn finding_failed(err: LibraryError) -> Error {
    match err {
        LibraryError::NotFound(id) => Error::NoSuchLibrary(id),
        LibraryError::Database(err) => Error::Database(err),
        LibraryError::NameBlank
        | LibraryError::NameTaken { .. }
        | LibraryError::FoldersEmpty
        | LibraryError::Folder { .. }
        | LibraryError::FolderTaken { .. }
        | LibraryError::FolderOverlaps { .. }
        | LibraryError::TitleSources { .. }
        | LibraryError::TitleSourceUnknown { .. }
        | LibraryError::NoTitleSourceSearches => {
            unreachable!("finding a Library refused a change: {err}")
        }
    }
}

#[cfg(test)]
#[path = "browse/browse_tests.rs"]
mod tests;
