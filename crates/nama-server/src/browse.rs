//! Browse: the Movies and Shows a Library holds, as a User sees them.
//!
//! [`Browse`] only reads. It finds titles through [`Contents`], never in the content tables
//! itself (see `AGENTS.md`), and attaches the User's [`WatchState`]s.

use std::sync::Arc;

use nama_plugin_api::metadata::ExternalId;

use crate::{
    contents::{Contents, Episode, Movie, Season, Show},
    db::Database,
    libraries::{self, Kind, Libraries},
    watch_states::{WatchState, WatchStates},
};

/// Why Browse found nothing, or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("there is no Library {0}")]
    NoSuchLibrary(i64),
    #[error("the Library isn't a {wanted:?} Library")]
    WrongKind { wanted: Kind },
    /// No Movie, Show or Episode in the Library has the id asked for.
    #[error("nothing in the Library is Matched to that id")]
    NothingMatched,
    #[error("the Show has no such Season in the Library")]
    NoSuchSeason,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Reads the Libraries' titles and the Users' Watch States of them.
#[derive(Debug, Clone)]
pub(crate) struct Browse {
    libraries: Libraries,
    contents: Contents,
    watch_states: WatchStates,
}

impl Browse {
    pub(crate) fn new(db: Arc<Database>) -> Self {
        Self {
            libraries: Libraries::new(Arc::clone(&db)),
            contents: Contents::new(Arc::clone(&db)),
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
        let movies = self.movies_of(library).await?;
        let ids: Vec<&ExternalId> = movies.iter().map(|movie| &movie.id).collect();
        let states = self.watch_states.of_movies(user, &ids).await?;
        Ok(movies.into_iter().zip(states).collect())
    }

    /// The Movie `id` of the Movies Library `library`, with the User `user`'s Watch State of it,
    /// if they've started it.
    // ponytail: whole-Library read (see AGENTS.md).
    pub(crate) async fn movie(
        &self,
        user: i64,
        library: i64,
        id: &ExternalId,
    ) -> Result<(Movie, Option<WatchState>), Error> {
        let movie = self
            .movies_of(library)
            .await?
            .into_iter()
            .find(|movie| &movie.id == id)
            .ok_or(Error::NothingMatched)?;
        let state = self.watch_states.of_movies(user, &[id]).await?.pop();
        Ok((movie, state.flatten()))
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
        let ids: Vec<&ExternalId> = episodes.iter().map(|episode| &episode.id).collect();
        let states = self.watch_states.of_episodes(user, &ids).await?;
        Ok(episodes.into_iter().zip(states).collect())
    }

    /// The Shows of the Shows Library `library`.
    // ponytail: whole-Library read (see AGENTS.md).
    pub(crate) async fn shows(&self, library: i64) -> Result<Vec<Show>, Error> {
        self.require_kind(library, Kind::Shows).await?;
        Ok(self.contents.shows(library).await?)
    }

    /// The Show `id` of the Shows Library `library`.
    pub(crate) async fn show(&self, library: i64, id: &ExternalId) -> Result<Show, Error> {
        self.shows(library)
            .await?
            .into_iter()
            .find(|show| &show.id == id)
            .ok_or(Error::NothingMatched)
    }

    /// Season `number` of the Show `show` of the Shows Library `library`.
    pub(crate) async fn season(
        &self,
        library: i64,
        show: &ExternalId,
        number: i64,
    ) -> Result<Season, Error> {
        self.show(library, show)
            .await?
            .seasons
            .remove(&number)
            .ok_or(Error::NoSuchSeason)
    }

    /// The Episode `id` of any Show of the Shows Library `library`.
    // ponytail: whole-Library read (see AGENTS.md).
    pub(crate) async fn episode(&self, library: i64, id: &ExternalId) -> Result<Episode, Error> {
        self.shows(library)
            .await?
            .into_iter()
            .flat_map(|show| show.seasons.into_values())
            .flat_map(|season| season.episodes)
            .find(|episode| &episode.id == id)
            .ok_or(Error::NothingMatched)
    }

    async fn movies_of(&self, library: i64) -> Result<Vec<Movie>, Error> {
        self.require_kind(library, Kind::Movies).await?;
        Ok(self.contents.movies(library).await?)
    }

    async fn require_kind(&self, library: i64, wanted: Kind) -> Result<(), Error> {
        let library = self.libraries.get(library).await.map_err(finding_failed)?;
        if library.kind == wanted {
            return Ok(());
        }
        Err(Error::WrongKind { wanted })
    }
}

/// What [`Libraries::get`] failing means to Browse.
#[expect(
    clippy::unreachable,
    reason = "`Libraries::get` only finds a Library; it never refuses a change"
)]
fn finding_failed(err: libraries::Error) -> Error {
    match err {
        libraries::Error::NotFound(id) => Error::NoSuchLibrary(id),
        libraries::Error::Database(err) => Error::Database(err),
        libraries::Error::Invalid(_) | libraries::Error::FolderTaken { .. } => {
            unreachable!("finding a Library refused a change: {err}")
        }
    }
}
