//! Browse: the Movies and Shows a Library holds, as a User sees them.
//!
//! [`Browse`] only reads. It finds titles through [`Contents`], never in the content tables
//! itself (see `AGENTS.md`), and attaches their Versions' Tracks and the User's [`WatchState`]s.

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::ExternalId;

use crate::{
    contents::{self, Contents, Found, Stored},
    db::Database,
    libraries::{self, Kind, Libraries, Library},
    tracks::{Probe, Track, Tracks},
    watch_states::{MovieOrEpisode, WatchState, WatchStates},
};

/// A Movie whose Versions have their Tracks.
pub(crate) type Movie = contents::Movie<Version>;
/// A Show whose Episodes' Versions have their Tracks.
pub(crate) type Show = contents::Show<Version>;
/// A Season whose Episodes' Versions have their Tracks.
pub(crate) type Season = contents::Season<Version>;
/// An Episode whose Versions have their Tracks.
pub(crate) type Episode = contents::Episode<Version>;

/// A stored file that a Movie or Episode is available in, with its Tracks.
#[derive(Debug, Clone)]
pub(crate) struct Version {
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
        episode: Episode,
        state: WatchState,
    },
}

/// A Library's titles, read whole.
enum Titles {
    Movies(Vec<Movie>),
    Shows(Vec<Show>),
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
        let movies = self.movies_of(library).await?;
        let ids: Vec<&ExternalId> = movies.iter().map(|movie| &movie.id).collect();
        let states = self.watch_states.of_movies(user, &ids).await?;
        Ok(movies.into_iter().zip(states).collect())
    }

    /// The Movie `id` of the Movies Library `library`, with the User `user`'s Watch State of it,
    /// if they've started it.
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
    pub(crate) async fn shows(&self, library: i64) -> Result<Vec<Show>, Error> {
        match self.read(&self.library(library).await?).await? {
            Titles::Shows(shows) => Ok(shows),
            Titles::Movies(_) => Err(Error::WrongKind {
                wanted: Kind::Shows,
            }),
        }
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
    pub(crate) async fn continue_watching(&self, user: i64) -> Result<Vec<Started>, Error> {
        let started = self.watch_states.continue_watching(user).await?;
        if started.is_empty() {
            return Ok(Vec::new());
        }
        let mut unfound: HashMap<MovieOrEpisode, WatchState> = started.iter().cloned().collect();
        let mut found = HashMap::new();
        // Oldest first, so a title in two Libraries is found in the older.
        for library in self.libraries.list().await.map_err(finding_failed)? {
            found.extend(match self.read(&library).await? {
                Titles::Movies(movies) => started_movies(library.id, movies, &mut unfound),
                Titles::Shows(shows) => started_episodes(library.id, &shows, &mut unfound),
            });
        }
        Ok(started
            .into_iter()
            .filter_map(|(watched, _)| found.remove(&watched))
            .collect())
    }

    /// The Movies, Shows and Episodes of every Library that a search for `query` finds: see
    /// [`Contents::search`].
    pub(crate) async fn search(&self, query: &str) -> Result<Found, Error> {
        Ok(self.contents.search(query).await?)
    }

    /// Every title of `library`, its Versions with the Tracks probing read from their files as
    /// they are now. Every finder reads through here.
    // ponytail: whole-Library read (see AGENTS.md).
    async fn read(&self, library: &Library) -> Result<Titles, Error> {
        Ok(match library.kind {
            Kind::Movies => {
                let movies = self.contents.movies(library.id).await?;
                let files = movies.iter().flat_map(|movie| &movie.versions);
                let probes = self.probes(files).await?;
                let with_tracks = |movie: contents::Movie| movie.map_versions(with_tracks(&probes));
                Titles::Movies(movies.into_iter().map(with_tracks).collect())
            }
            Kind::Shows => {
                let shows = self.contents.shows(library.id).await?;
                let files = (shows.iter().flat_map(contents::Show::episodes))
                    .flat_map(|episode| &episode.versions);
                let probes = self.probes(files).await?;
                let with_tracks = |show: contents::Show| show.map_versions(with_tracks(&probes));
                Titles::Shows(shows.into_iter().map(with_tracks).collect())
            }
        })
    }

    /// What probing read from the files of `versions` as they are now, by file id.
    async fn probes(
        &self,
        versions: impl Iterator<Item = &contents::Version>,
    ) -> Result<HashMap<i64, Probe>, sqlx::Error> {
        let files: Vec<Stored> = versions.map(|version| version.file).collect();
        self.tracks.of(&files).await
    }

    async fn movies_of(&self, library: i64) -> Result<Vec<Movie>, Error> {
        match self.read(&self.library(library).await?).await? {
            Titles::Movies(movies) => Ok(movies),
            Titles::Shows(_) => Err(Error::WrongKind {
                wanted: Kind::Movies,
            }),
        }
    }

    async fn library(&self, library: i64) -> Result<Library, Error> {
        self.libraries.get(library).await.map_err(finding_failed)
    }
}

/// Makes a Version from Contents into one with the Tracks `probes` has of its file.
fn with_tracks(probes: &HashMap<i64, Probe>) -> impl Fn(contents::Version) -> Version {
    |version| {
        // A file holding two Episodes is a Version of each, so its Tracks are cloned.
        let tracks = (probes.get(&version.file.id))
            .map(|probe| probe.tracks.clone())
            .unwrap_or_default();
        Version {
            file: version.file,
            path: version.path,
            tracks,
        }
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
        for episode in show.episodes() {
            let watched = MovieOrEpisode::episode(&episode.id);
            let Some(state) = unfound.remove(&watched) else {
                continue;
            };
            // ponytail: a Show cloned per started Episode; `Arc<Show>` if a profile ever
            // points here.
            let episode = Started::Episode {
                library,
                show: show.clone(),
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
        contents::{Change, Contents, Decided, Item, Metadata, Stored},
        db::Database,
        libraries::{Kind, Libraries},
        plugins::Registry,
        tracks::{Probe, Track, TrackKind, Tracks},
        users::{Permissions, User, Users},
        watch_states::{Progress, WatchStates},
    };

    const ALIEN_TMDB_ID: i64 = 348;
    const KNIGHT_TMDB_ID: i64 = 224_372;
    const KNIGHT_PILOT_TMDB_ID: i64 = 5_000_001;

    /// Creates a Library of `kind` on `folder`, with its canonical path.
    async fn library(
        db: &Arc<Database>,
        name: &str,
        kind: Kind,
        folder: &TempDir,
    ) -> (i64, String) {
        let root = folder.path().canonicalize().unwrap();
        let root = root.to_str().unwrap().to_owned();
        let library = Libraries::new(Arc::clone(db))
            .create(name, kind, vec![root.clone()])
            .await
            .unwrap()
            .id;
        (library, root)
    }

    /// Stores the file at `path` in `library`, holding `item`, and Matches the Item to `matched`.
    async fn store_matched(
        db: &Arc<Database>,
        library: i64,
        path: String,
        item: Item,
        matched: Metadata,
    ) {
        let contents = Contents::new(Arc::clone(db));
        let file = Change::Store {
            path,
            size: 10,
            modified_ns: 0,
            items: vec![item],
        };
        contents.apply(library, vec![file]).await.unwrap();
        let (undecided, _, _) = contents.undecided(library).await.unwrap().remove(0);
        contents
            .record(vec![(undecided, Decided::Matched(Box::new(matched)))])
            .await
            .unwrap();
    }

    /// TMDB's metadata of the title `title`, which has the id `tmdb`.
    fn tmdb_metadata(title: &str, tmdb: i64) -> Metadata {
        Metadata {
            plugin: PluginId::from_static("tmdb"),
            ids: ExternalIds {
                tmdb: Some(tmdb),
                ..ExternalIds::default()
            },
            show_ids: ExternalIds::default(),
            title: title.to_owned(),
            original_title: None,
            show_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        }
    }

    /// Creates a Movies Library on a folder of its own, with one file Matched to Alien.
    async fn library_with_alien(db: &Arc<Database>, name: &str, folder: &TempDir) -> i64 {
        let (library, root) = library(db, name, Kind::Movies, folder).await;
        let alien = Item::Movie {
            title: "Alien".to_owned(),
            year: Some(1979),
            hint: None,
        };
        let path = format!("{root}/Alien (1979)/Alien (1979).mkv");
        store_matched(
            db,
            library,
            path,
            alien,
            tmdb_metadata("Alien", ALIEN_TMDB_ID),
        )
        .await;
        library
    }

    /// The one file stored in `library`, as stored now.
    async fn only_file(db: &Arc<Database>, library: i64) -> Stored {
        let stored = Contents::new(Arc::clone(db)).stored(library).await.unwrap();
        *stored.values().next().unwrap()
    }

    fn stereo() -> Track {
        Track {
            codec: "ac3".to_owned(),
            language: None,
            title: None,
            default: false,
            forced: false,
            sdh: false,
            kind: TrackKind::Audio { channels: Some(2) },
        }
    }

    fn probe_of(tracks: Vec<Track>) -> Probe {
        Probe {
            container: "matroska,webm".to_owned(),
            duration_ms: None,
            bitrate: None,
            tracks,
        }
    }

    #[tokio::test]
    async fn an_episode_has_the_same_tracks_however_it_is_reached() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let (library, root) = library(&db, "Shows", Kind::Shows, &folder).await;
        let pilot_file = Item::Episode {
            show: "A Knight of the Seven Kingdoms".to_owned(),
            year: Some(2026),
            hint: None,
            season: 1,
            episode: 1,
        };
        let matched = Metadata {
            show_ids: ExternalIds {
                tmdb: Some(KNIGHT_TMDB_ID),
                ..ExternalIds::default()
            },
            show_title: Some("A Knight of the Seven Kingdoms".to_owned()),
            ..tmdb_metadata("The Hedge Knight", KNIGHT_PILOT_TMDB_ID)
        };
        let path = format!("{root}/A Knight of the Seven Kingdoms/Season 01/S01E01.mkv");
        store_matched(&db, library, path, pilot_file, matched).await;
        let file = only_file(&db, library).await;
        Tracks::new(Arc::clone(&db))
            .store(file, probe_of(vec![stereo()]))
            .await
            .unwrap();
        let browse = Browse::new(db);
        let (show, pilot) = (
            ExternalId::Tmdb(KNIGHT_TMDB_ID),
            ExternalId::Tmdb(KNIGHT_PILOT_TMDB_ID),
        );

        let (listed, _) = browse
            .episodes(0, library, &show, 1)
            .await
            .unwrap()
            .remove(0);
        let in_season = browse
            .season(library, &show, 1)
            .await
            .unwrap()
            .episodes
            .remove(0);
        let in_show = browse.show(library, &show).await.unwrap();
        let in_show = in_show.episodes().next().unwrap().clone();
        let by_id = browse.episode(library, &pilot).await.unwrap();

        let tracks = [listed, in_season, in_show, by_id].map(|episode| {
            assert_eq!(episode.id, pilot);
            episode.versions[0].tracks.clone()
        });
        assert_eq!(tracks, [[stereo()], [stereo()], [stereo()], [stereo()]]);
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

    #[tokio::test]
    async fn continue_watching_reads_no_library_for_a_user_who_started_nothing() {
        let (data, unreadable_data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let unreadable = Arc::new(Database::open(unreadable_data.path()).await.unwrap());
        unreadable.reader().close().await;
        let browse = Browse {
            libraries: Libraries::new(Arc::clone(&unreadable)),
            contents: Contents::new(Arc::clone(&unreadable)),
            tracks: Tracks::new(unreadable),
            watch_states: WatchStates::new(db),
        };

        let started = browse.continue_watching(1).await.unwrap();

        assert!(started.is_empty(), "{started:?}");
    }

    #[tokio::test]
    async fn a_version_has_no_tracks_read_from_its_file_before_it_changed() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let library = library_with_alien(&db, "Movies", &folder).await;
        let file = only_file(&db, library).await;
        let before_it_changed = Stored {
            modified_ns: file.modified_ns - 1,
            ..file
        };
        Tracks::new(Arc::clone(&db))
            .store(before_it_changed, probe_of(vec![stereo()]))
            .await
            .unwrap();

        let (movie, _) = Browse::new(db)
            .movie(0, library, &ExternalId::Tmdb(ALIEN_TMDB_ID))
            .await
            .unwrap();

        assert_eq!(movie.versions[0].tracks, []);
    }
}
