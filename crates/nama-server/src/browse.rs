//! Browse: the Movies and Shows a Library holds, as a User sees them.
//!
//! [`Browse`] only reads. It finds titles through [`Contents`], never in the content tables
//! itself (see `AGENTS.md`), and attaches their Versions' Tracks and the User's [`WatchState`]s.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::ExternalId;

use crate::{
    contents::{Contents, Episode, Movie, Season, Show, Version},
    db::Database,
    libraries::{self, Kind, Libraries, Library},
    tracks::Tracks,
    watch_states::{MovieOrEpisode, WatchState, WatchStates},
};

/// Why Browse found nothing, or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
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
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// A Movie or Episode a User has started and not played, in the Library Continue Watching lists
/// it under.
#[derive(Debug)]
pub(crate) enum Started {
    Movie {
        library: i64,
        movie: Movie,
        state: WatchState,
    },
    Episode {
        library: i64,
        show: Show,
        /// The Episode's season number.
        season: i64,
        episode: Episode,
        state: WatchState,
    },
}

/// Reads the Libraries' titles and the Users' Watch States of them.
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
        let mut movies = self.movies_of(library).await?;
        self.add_tracks(movies.iter_mut().flat_map(|movie| &mut movie.versions))
            .await?;
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
        let mut movie = self
            .movies_of(library)
            .await?
            .into_iter()
            .find(|movie| &movie.id == id)
            .ok_or(Error::NothingMatched)?;
        self.add_tracks(&mut movie.versions).await?;
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
        let mut episodes = self.season(library, show, season).await?.episodes;
        self.add_tracks(
            episodes
                .iter_mut()
                .flat_map(|episode| &mut episode.versions),
        )
        .await?;
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

    /// The User `user`'s Continue Watching: the Movies and Episodes they've started and not
    /// played, most recent first. Each is in the oldest Library that has it, and one no Library
    /// has is left out.
    // ponytail: whole-Library read (see AGENTS.md).
    pub(crate) async fn continue_watching(&self, user: i64) -> Result<Vec<Started>, Error> {
        let started = self.watch_states.continue_watching(user).await?;
        let mut unfound: HashMap<MovieOrEpisode, WatchState> = started.iter().cloned().collect();
        let mut found = HashMap::new();
        // Oldest first, so a title in two Libraries is found in the older.
        for library in self.libraries.list().await.map_err(finding_failed)? {
            found.extend(self.started_in(&library, &mut unfound).await?);
        }
        let mut started: Vec<Started> = started
            .into_iter()
            .filter_map(|(watched, _)| found.remove(&watched))
            .collect();
        self.add_tracks(started.iter_mut().flat_map(|started| match started {
            Started::Movie { movie, .. } => &mut movie.versions,
            Started::Episode { episode, .. } => &mut episode.versions,
        }))
        .await?;
        Ok(started)
    }

    /// Fills in the Tracks of each of `versions` that has been probed.
    async fn add_tracks(
        &self,
        versions: impl IntoIterator<Item = &mut Version>,
    ) -> Result<(), sqlx::Error> {
        let mut versions: Vec<&mut Version> = versions.into_iter().collect();
        let ids: Vec<i64> = versions.iter().map(|version| version.id).collect();
        let probes = self.tracks.of(&ids).await?;
        for version in &mut versions {
            if let Some(probe) = probes.get(&version.id) {
                // A file holding two Episodes is a Version of each.
                version.tracks.clone_from(&probe.tracks);
            }
        }
        Ok(())
    }

    /// The Movies or Episodes of `library` that `unfound` has the Watch State of, each taken out
    /// of `unfound`.
    async fn started_in(
        &self,
        library: &Library,
        unfound: &mut HashMap<MovieOrEpisode, WatchState>,
    ) -> Result<Vec<(MovieOrEpisode, Started)>, Error> {
        Ok(match library.kind {
            Kind::Movies => {
                let movies = self.contents.movies(library.id).await?;
                started_movies(library.id, movies, unfound)
            }
            Kind::Shows => {
                let shows = self.contents.shows(library.id).await?;
                started_episodes(library.id, &shows, unfound)
            }
        })
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

/// The `movies` of the Library `library` that `unfound` has the Watch State of, each taken out
/// of `unfound`.
fn started_movies(
    library: i64,
    movies: Vec<Movie>,
    unfound: &mut HashMap<MovieOrEpisode, WatchState>,
) -> Vec<(MovieOrEpisode, Started)> {
    movies
        .into_iter()
        .filter_map(|movie| {
            let watched = MovieOrEpisode::movie(&movie.id);
            let state = unfound.remove(&watched)?;
            Some((
                watched,
                Started::Movie {
                    library,
                    movie,
                    state,
                },
            ))
        })
        .collect()
}

/// The Episodes of `shows`, of the Library `library`, that `unfound` has the Watch State of, each
/// taken out of `unfound`.
fn started_episodes(
    library: i64,
    shows: &[Show],
    unfound: &mut HashMap<MovieOrEpisode, WatchState>,
) -> Vec<(MovieOrEpisode, Started)> {
    let mut started = Vec::new();
    for show in shows {
        let episodes = show.seasons.iter().flat_map(|(&number, season)| {
            season.episodes.iter().map(move |episode| (number, episode))
        });
        for (season, episode) in episodes {
            let watched = MovieOrEpisode::episode(&episode.id);
            let Some(state) = unfound.remove(&watched) else {
                continue;
            };
            // ponytail: a Show cloned per started Episode; `Arc<Show>` if a profile ever
            // points here.
            let episode = Started::Episode {
                library,
                show: show.clone(),
                season,
                episode: episode.clone(),
                state,
            };
            started.push((watched, episode));
        }
    }
    started
}

/// What [`Libraries::get`] or [`Libraries::list`] failing means to Browse.
#[expect(
    clippy::unreachable,
    reason = "`Libraries::get` and `Libraries::list` only find Libraries; they never refuse a \
              change"
)]
fn finding_failed(err: libraries::Error) -> Error {
    match err {
        libraries::Error::NotFound(id) => Error::NoSuchLibrary(id),
        libraries::Error::Database(err) => Error::Database(err),
        libraries::Error::NameBlank
        | libraries::Error::FoldersEmpty
        | libraries::Error::Folder { .. }
        | libraries::Error::FolderTaken { .. } => {
            unreachable!("finding a Library refused a change: {err}")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nama_plugin_api::{
        PluginId,
        metadata::{ExternalId, ExternalIds},
    };
    use tempfile::TempDir;

    use super::{Browse, Started};
    use crate::{
        contents::{Change, Contents, Decided, Item, Metadata},
        db::Database,
        libraries::{Kind, Libraries},
        plugins::Registry,
        users::{Permissions, User, Users},
        watch_states::{Progress, WatchStates},
    };

    const ALIEN_TMDB_ID: i64 = 348;

    /// Creates a Movies Library on a folder of its own, with one file Matched to Alien.
    async fn library_with_alien(db: &Arc<Database>, name: &str, folder: &TempDir) -> i64 {
        let root = folder.path().canonicalize().unwrap();
        let root = root.to_str().unwrap();
        let library = Libraries::new(Arc::clone(db))
            .create(name, Kind::Movies, vec![root.to_owned()])
            .await
            .unwrap()
            .id;
        let contents = Contents::new(Arc::clone(db));
        let file = Change::Store {
            path: format!("{root}/Alien (1979)/Alien (1979).mkv"),
            size: 10,
            modified_ns: 0,
            items: vec![Item::Movie {
                title: "Alien".to_owned(),
                year: Some(1979),
                hint: None,
            }],
        };
        contents.apply(library, vec![file]).await.unwrap();
        let (item, _, _) = contents.undecided(library).await.unwrap().remove(0);
        let alien = Metadata {
            plugin: PluginId::from_static("tmdb"),
            ids: ExternalIds {
                tmdb: Some(ALIEN_TMDB_ID),
                ..ExternalIds::default()
            },
            show_ids: ExternalIds::default(),
            title: "Alien".to_owned(),
            original_title: None,
            show_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        };
        contents
            .record(vec![(item, Decided::Matched(Box::new(alien)))])
            .await
            .unwrap();
        library
    }

    #[tokio::test]
    async fn a_movie_matched_in_two_libraries_is_listed_in_continue_watching_under_the_older_one() {
        let data = tempfile::tempdir().unwrap();
        let (older_folder, newer_folder) =
            (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let older = library_with_alien(&db, "Older", &older_folder).await;
        library_with_alien(&db, "Newer", &newer_folder).await;
        let root = User {
            id: 0,
            name: "Root".to_owned(),
            permissions: Permissions::ADMIN,
            created_at: 0,
        };
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
            .unwrap();
        let user = Users::new(Arc::clone(&db), plugins)
            .create(&root, "Ada", "correct horse battery staple")
            .await
            .unwrap();
        let halfway = Progress::new(1_000, 2_000).unwrap();
        WatchStates::new(Arc::clone(&db))
            .report_movie(user, &ExternalId::Tmdb(ALIEN_TMDB_ID), halfway)
            .await
            .unwrap();

        let started = Browse::new(db).continue_watching(user).await.unwrap();

        let listed: Vec<_> = started
            .iter()
            .map(|started| match started {
                Started::Movie { library, movie, .. } => (*library, movie.id.clone()),
                Started::Episode { .. } => panic!("Alien is a Movie"),
            })
            .collect();
        assert_eq!(listed, [(older, ExternalId::Tmdb(ALIEN_TMDB_ID))]);
    }
}
