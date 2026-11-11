//! Browse: the Movies and Shows a Library holds, as a User sees them.
//!
//! [`Browse`] finds titles through [`Contents`], never in the content tables itself (see
//! `AGENTS.md`), and attaches their Versions' Tracks and the User's [`WatchState`]s. It writes only
//! Watch State, for what a reported Version or a marked title stands for (ADR 0013).

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::ExternalId;

use crate::{
    contents::{self, Contents, Found, MatchedTo, Stored},
    db::Database,
    libraries::{self, Kind, Libraries, Library},
    tracks::{Probe, Track, Tracks},
    watch_states::{MovieOrEpisode, Progress, WatchState, WatchStates},
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
        Kind::Movies => "Movies",
        Kind::Shows => "Shows",
    })]
    WrongKind { wanted: Kind },
    /// No Movie, Show or Episode in the Library has the id asked for.
    #[error("nothing in the Library is Matched to that id")]
    NothingMatched,
    #[error("the Show has no such Season in the Library")]
    NoSuchSeason,
    #[error("there is no such Version")]
    NoSuchVersion,
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

/// What a User marks played or unplayed: see [`Browse::mark`].
#[derive(Debug)]
pub(crate) enum Target {
    Movie(ExternalId),
    Episode(ExternalId),
    Season { show: ExternalId, number: i64 },
    Show(ExternalId),
}

/// A Library's titles, read whole, with Versions of the form `V`.
enum Titles<V = Version> {
    Movies(Vec<contents::Movie<V>>),
    Shows(Vec<contents::Show<V>>),
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
        let movies = self.read(&self.library(library).await?).await?.movies()?;
        let ids: Vec<_> = (movies.iter())
            .map(|movie| MovieOrEpisode::movie(&movie.id))
            .collect();
        let states = self.watch_states.of(user, &ids).await?;
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
        let movie = self.read(&self.library(library).await?).await?.movie(id)?;
        let state = self
            .watch_states
            .of(user, &[MovieOrEpisode::movie(id)])
            .await?
            .pop();
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
        let ids: Vec<_> = (episodes.iter())
            .map(|episode| MovieOrEpisode::episode(&episode.id))
            .collect();
        let states = self.watch_states.of(user, &ids).await?;
        Ok(episodes.into_iter().zip(states).collect())
    }

    /// The Shows of the Shows Library `library`.
    pub(crate) async fn shows(&self, library: i64) -> Result<Vec<Show>, Error> {
        self.read(&self.library(library).await?).await?.shows()
    }

    /// The Show `id` of the Shows Library `library`.
    pub(crate) async fn show(&self, library: i64, id: &ExternalId) -> Result<Show, Error> {
        self.read(&self.library(library).await?).await?.show(id)
    }

    /// Season `number` of the Show `show` of the Shows Library `library`.
    pub(crate) async fn season(
        &self,
        library: i64,
        show: &ExternalId,
        number: i64,
    ) -> Result<Season, Error> {
        self.read(&self.library(library).await?)
            .await?
            .season(show, number)
    }

    /// Records, as of now, that the User `user` has got as far as `progress` through the Version
    /// `version`: through its Movie, or split across each Episode its file holds. An Unmatched
    /// Item has no Watch State, so progress on it stores nothing (ADR 0013).
    pub(crate) async fn report(
        &self,
        user: i64,
        version: i64,
        progress: Progress,
    ) -> Result<(), Error> {
        let titles = match self.contents.matched_to(version).await? {
            MatchedTo::NoSuchVersion => return Err(Error::NoSuchVersion),
            MatchedTo::Nothing => return Ok(()),
            MatchedTo::Movie(movie) => vec![Some(MovieOrEpisode::movie(&movie))],
            // An Item that isn't Matched keeps its share of the file, so the others' shares don't
            // move.
            MatchedTo::Episodes(episodes) => (episodes.iter())
                .map(|episode| episode.as_ref().map(MovieOrEpisode::episode))
                .collect(),
        };
        Ok(self.watch_states.report(user, titles, progress).await?)
    }

    /// Marks, as of now, `target` of the Library `library` played or unplayed for the User
    /// `user`: a Season or Show is each of its Episodes. Fails as browsing to it does if the
    /// Library has no such title.
    pub(crate) async fn mark(
        &self,
        user: i64,
        library: i64,
        target: Target,
        played: bool,
    ) -> Result<(), Error> {
        let titles = self.titles(&self.library(library).await?).await?;
        let episode = |episode: &contents::Episode| MovieOrEpisode::episode(&episode.id);
        let marked = match target {
            Target::Movie(id) => vec![MovieOrEpisode::movie(&titles.movie(&id)?.id)],
            Target::Episode(id) => vec![episode(&titles.episode(&id)?)],
            Target::Season { show, number } => (titles.season(&show, number)?.episodes.iter())
                .map(episode)
                .collect(),
            Target::Show(id) => titles.show(&id)?.episodes().map(episode).collect(),
        };
        Ok(self.watch_states.mark(user, marked, played).await?)
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
    /// they are now.
    async fn read(&self, library: &Library) -> Result<Titles, Error> {
        Ok(match self.titles(library).await? {
            Titles::Movies(movies) => {
                let files = movies.iter().flat_map(|movie| &movie.versions);
                let probes = self.probes(files).await?;
                let tracked = |movie: contents::Movie| movie.map_versions(with_tracks(&probes));
                Titles::Movies(movies.into_iter().map(tracked).collect())
            }
            Titles::Shows(shows) => {
                let files = (shows.iter().flat_map(contents::Show::episodes))
                    .flat_map(|episode| &episode.versions);
                let probes = self.probes(files).await?;
                let tracked = |show: contents::Show| show.map_versions(with_tracks(&probes));
                Titles::Shows(shows.into_iter().map(tracked).collect())
            }
        })
    }

    /// Every title of `library`, as [`Contents`] has it, without Tracks. Every finder reads
    /// through here.
    // ponytail: whole-Library read (see AGENTS.md).
    async fn titles(&self, library: &Library) -> Result<Titles<contents::Version>, Error> {
        Ok(match library.kind {
            Kind::Movies => Titles::Movies(self.contents.movies(library.id).await?),
            Kind::Shows => Titles::Shows(self.contents.shows(library.id).await?),
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

    async fn library(&self, library: i64) -> Result<Library, Error> {
        self.libraries.get(library).await.map_err(finding_failed)
    }
}

impl<V> Titles<V> {
    fn movies(self) -> Result<Vec<contents::Movie<V>>, Error> {
        match self {
            Self::Movies(movies) => Ok(movies),
            Self::Shows(_) => Err(Error::WrongKind {
                wanted: Kind::Movies,
            }),
        }
    }

    fn shows(self) -> Result<Vec<contents::Show<V>>, Error> {
        match self {
            Self::Shows(shows) => Ok(shows),
            Self::Movies(_) => Err(Error::WrongKind {
                wanted: Kind::Shows,
            }),
        }
    }

    fn movie(self, id: &ExternalId) -> Result<contents::Movie<V>, Error> {
        (self.movies()?.into_iter())
            .find(|movie| &movie.id == id)
            .ok_or(Error::NothingMatched)
    }

    fn show(self, id: &ExternalId) -> Result<contents::Show<V>, Error> {
        (self.shows()?.into_iter())
            .find(|show| &show.id == id)
            .ok_or(Error::NothingMatched)
    }

    fn season(self, show: &ExternalId, number: i64) -> Result<contents::Season<V>, Error> {
        (self.show(show)?.seasons.remove(&number)).ok_or(Error::NoSuchSeason)
    }

    /// The Episode `id` of any of its Shows.
    fn episode(self, id: &ExternalId) -> Result<contents::Episode<V>, Error> {
        (self.shows()?.into_iter())
            .flat_map(|show| show.seasons.into_values())
            .flat_map(|season| season.episodes)
            .find(|episode| &episode.id == id)
            .ok_or(Error::NothingMatched)
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

    use nama_plugin_api::metadata::{ExternalId, ExternalIds};
    use tempfile::TempDir;

    use super::{Browse, Error, Started, Target};
    use crate::{
        contents::{
            Change, Contents, Decided, Item, Metadata, Stored,
            fixture::{MatchedMovie, decide, tmdb_movie},
        },
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
    const KNIGHT_SECOND_TMDB_ID: i64 = 5_000_002;
    const KNIGHT_SEASON_2_TMDB_ID: i64 = 5_000_003;

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

    /// Stores the file at `path` in `library`, holding each Item of `items` with what was decided
    /// about it, and returns its id, the id of its Version.
    async fn store(
        db: &Arc<Database>,
        library: i64,
        path: String,
        items: Vec<(Item, Decided)>,
    ) -> i64 {
        let contents = Contents::new(Arc::clone(db));
        let (items, decided): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        let file = Change::Store {
            path: path.clone(),
            size: 10,
            modified_ns: 0,
            items,
        };
        contents.apply(library, vec![file]).await.unwrap();
        let (version, _items) = decide(&contents, library, &path, decided).await;
        version
    }

    /// Stores the file at `path` in `library`, holding `item`, and Matches the Item to `matched`.
    async fn store_matched(
        db: &Arc<Database>,
        library: i64,
        path: String,
        item: Item,
        matched: Metadata,
    ) -> i64 {
        let matched = Decided::Matched(Box::new(matched));
        store(db, library, path, vec![(item, matched)]).await
    }

    /// Ada, a User of `db`.
    async fn ada(db: &Arc<Database>) -> i64 {
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
        Users::new(Arc::clone(db), plugins.extension_points())
            .create(&root, "Ada", "correct horse battery staple")
            .await
            .unwrap()
    }

    /// A Movies Library on `folder` with one file, Matched to Alien.
    async fn library_with_alien(db: &Arc<Database>, folder: &TempDir) -> MatchedMovie {
        MatchedMovie::new(db, folder.path(), tmdb_movie(ALIEN_TMDB_ID, "Alien")).await
    }

    fn knight_episode(season: i64, episode: i64) -> Item {
        Item::Episode {
            show: "A Knight of the Seven Kingdoms".to_owned(),
            year: Some(2026),
            hint: None,
            season,
            episode,
        }
    }

    /// A Match of an Episode of A Knight of the Seven Kingdoms with the TMDB id `tmdb`.
    fn knight_match(tmdb: i64) -> Decided {
        Decided::Matched(Box::new(Metadata {
            show_ids: ExternalIds {
                tmdb: Some(KNIGHT_TMDB_ID),
                ..ExternalIds::default()
            },
            show_title: Some("A Knight of the Seven Kingdoms".to_owned()),
            ..tmdb_movie(tmdb, "An Episode")
        }))
    }

    /// Creates a Shows Library holding A Knight of the Seven Kingdoms' Season 1 Episodes 1 and 2,
    /// in one file, and Season 2 Episode 1, and returns the Library and the first file's id.
    async fn library_with_knight(db: &Arc<Database>, folder: &TempDir) -> (i64, i64) {
        let (library, root) = library(db, "Shows", Kind::Shows, folder).await;
        let show = format!("{root}/A Knight of the Seven Kingdoms");
        let both = vec![
            (knight_episode(1, 1), knight_match(KNIGHT_PILOT_TMDB_ID)),
            (knight_episode(1, 2), knight_match(KNIGHT_SECOND_TMDB_ID)),
        ];
        let two_episodes = store(
            db,
            library,
            format!("{show}/Season 01/S01E01-E02.mkv"),
            both,
        )
        .await;
        let path = format!("{show}/Season 02/S02E01.mkv");
        let second_season = vec![(knight_episode(2, 1), knight_match(KNIGHT_SEASON_2_TMDB_ID))];
        store(db, library, path, second_season).await;
        (library, two_episodes)
    }

    /// How far the User `user` has got through each Episode of A Knight of the Seven Kingdoms in
    /// `library`, by Season, and whether they've played it: `None` for one they haven't started.
    async fn knight_states(
        browse: &Browse,
        user: i64,
        library: i64,
    ) -> Vec<Vec<Option<(i64, bool)>>> {
        let mut states = Vec::new();
        for number in [1, 2] {
            let episodes = browse
                .episodes(user, library, &ExternalId::Tmdb(KNIGHT_TMDB_ID), number)
                .await
                .unwrap();
            let season = episodes
                .into_iter()
                .map(|(_, state)| state.map(|state| (state.position_ms, state.played)));
            states.push(season.collect());
        }
        states
    }

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
            ..tmdb_movie(KNIGHT_PILOT_TMDB_ID, "The Hedge Knight")
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

        let tracks = [listed, in_season, in_show].map(|episode| {
            assert_eq!(episode.id, pilot);
            episode.versions[0].tracks.clone()
        });
        assert_eq!(tracks, [[stereo()], [stereo()], [stereo()]]);
    }

    #[tokio::test]
    async fn a_movie_matched_in_two_libraries_is_listed_in_continue_watching_under_the_older_one() {
        let data = tempfile::tempdir().unwrap();
        let (older_folder, newer_folder) =
            (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let older = library_with_alien(&db, &older_folder).await;
        library_with_alien(&db, &newer_folder).await;
        let user = ada(&db).await;
        let browse = Browse::new(db);
        let halfway = Progress::new(1_000, 2_000).unwrap();
        browse.report(user, older.version, halfway).await.unwrap();

        let started = browse.continue_watching(user).await.unwrap();

        let listed: Vec<_> = started
            .iter()
            .map(|started| match started {
                Started::Movie { library, movie, .. } => (*library, movie.id.clone()),
                Started::Episode { .. } => panic!("Alien is a Movie"),
            })
            .collect();
        assert_eq!(listed, [(older.library, ExternalId::Tmdb(ALIEN_TMDB_ID))]);
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
        let library = library_with_alien(&db, &folder).await.library;
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

    #[tokio::test]
    async fn progress_on_a_movies_version_is_progress_through_the_movie() {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let MatchedMovie {
            library,
            version: alien,
            ..
        } = library_with_alien(&db, &folder).await;
        let user = ada(&db).await;
        let browse = Browse::new(db);

        let halfway = Progress::new(1_000, 2_000).unwrap();
        browse.report(user, alien, halfway).await.unwrap();

        let (_, state) = browse
            .movie(user, library, &ExternalId::Tmdb(ALIEN_TMDB_ID))
            .await
            .unwrap();
        let state = state.unwrap();
        assert_eq!((state.position_ms, state.duration_ms), (1_000, 2_000));
    }

    #[tokio::test]
    async fn progress_on_a_file_holding_two_episodes_is_split_across_them() {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let (library, two_episodes) = library_with_knight(&db, &folder).await;
        let user = ada(&db).await;
        let browse = Browse::new(db);

        let three_quarters = Progress::new(1_500, 2_000).unwrap();
        browse
            .report(user, two_episodes, three_quarters)
            .await
            .unwrap();

        assert_eq!(
            knight_states(&browse, user, library).await,
            [vec![Some((1_000, true)), Some((500, false))], vec![None]]
        );
    }

    #[tokio::test]
    async fn progress_on_an_unmatched_item_stores_nothing() {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let (library, root) = library(&db, "Movies", Kind::Movies, &folder).await;
        let unknown = Item::Movie {
            title: "Unknown".to_owned(),
            year: None,
            hint: None,
        };
        let unmatched = vec![(unknown, Decided::Unmatched(None))];
        let file = store(&db, library, format!("{root}/Unknown.mkv"), unmatched).await;
        let user = ada(&db).await;

        let halfway = Progress::new(1_000, 2_000).unwrap();
        Browse::new(Arc::clone(&db))
            .report(user, file, halfway)
            .await
            .unwrap();

        // Watch States' own Continue Watching lists every started title, in a Library or not.
        let started = WatchStates::new(db).continue_watching(user).await.unwrap();
        assert!(started.is_empty(), "{started:?}");
    }

    #[tokio::test]
    async fn progress_on_a_version_that_doesnt_exist_is_refused() {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let user = ada(&db).await;

        let halfway = Progress::new(1_000, 2_000).unwrap();
        let reported = Browse::new(db).report(user, 1, halfway).await;

        assert!(
            matches!(reported, Err(Error::NoSuchVersion)),
            "{reported:?}"
        );
    }

    #[tokio::test]
    async fn a_movie_marked_played_stays_played_until_marked_unplayed() {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let library = library_with_alien(&db, &folder).await.library;
        let user = ada(&db).await;
        let browse = Browse::new(db);
        let alien = ExternalId::Tmdb(ALIEN_TMDB_ID);
        let played = async || {
            let (_, state) = browse.movie(user, library, &alien).await.unwrap();
            state.map(|state| state.played)
        };

        let target = || Target::Movie(alien.clone());
        browse.mark(user, library, target(), true).await.unwrap();
        assert_eq!(played().await, Some(true));

        browse.mark(user, library, target(), false).await.unwrap();
        assert_eq!(played().await, None);
    }

    #[tokio::test]
    async fn marking_an_episode_a_season_or_a_show_marks_each_of_its_episodes() {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let (library, _) = library_with_knight(&db, &folder).await;
        let user = ada(&db).await;
        let browse = Browse::new(db);
        let show = ExternalId::Tmdb(KNIGHT_TMDB_ID);
        let played = Some((0, true));

        let pilot = Target::Episode(ExternalId::Tmdb(KNIGHT_PILOT_TMDB_ID));
        browse.mark(user, library, pilot, true).await.unwrap();
        assert_eq!(
            knight_states(&browse, user, library).await,
            [vec![played, None], vec![None]]
        );

        let first_season = Target::Season {
            show: show.clone(),
            number: 1,
        };
        browse
            .mark(user, library, first_season, true)
            .await
            .unwrap();
        assert_eq!(
            knight_states(&browse, user, library).await,
            [vec![played, played], vec![None]]
        );

        browse
            .mark(user, library, Target::Show(show.clone()), true)
            .await
            .unwrap();
        assert_eq!(
            knight_states(&browse, user, library).await,
            [vec![played, played], vec![played]]
        );

        browse
            .mark(user, library, Target::Show(show), false)
            .await
            .unwrap();
        assert_eq!(
            knight_states(&browse, user, library).await,
            [vec![None, None], vec![None]]
        );
    }

    #[tokio::test]
    async fn marking_a_title_the_library_doesnt_have_is_refused_as_browsing_to_it_is() {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let (library, _) = library_with_knight(&db, &folder).await;
        let user = ada(&db).await;
        let browse = Browse::new(db);
        let (show, elsewhere) = (ExternalId::Tmdb(KNIGHT_TMDB_ID), ExternalId::Tmdb(1));

        let marks = [
            Target::Episode(elsewhere.clone()),
            Target::Show(elsewhere),
            Target::Season { show, number: 3 },
            Target::Movie(ExternalId::Tmdb(ALIEN_TMDB_ID)),
        ];
        let mut refused = Vec::new();
        for target in marks {
            match browse.mark(user, library, target, true).await {
                Err(err) => refused.push(err.to_string()),
                Ok(()) => panic!("marked a title the Library doesn't have"),
            }
        }

        assert_eq!(
            refused,
            [
                Error::NothingMatched.to_string(),
                Error::NothingMatched.to_string(),
                Error::NoSuchSeason.to_string(),
                Error::WrongKind {
                    wanted: Kind::Movies
                }
                .to_string(),
            ]
        );
        assert_eq!(
            knight_states(&browse, user, library).await,
            [vec![None, None], vec![None]]
        );
    }
}
