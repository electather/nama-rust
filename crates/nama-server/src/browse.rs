//! Browse: the Movies and Shows a Library holds, and the pages of those and of Found ones, as a
//! User sees them.
//!
//! [`Browse`] finds titles through [`Contents`], never in the content tables itself (see
//! `src/library/AGENTS.md`), and attaches the User's [`WatchState`]s and, only to the Movies and
//! Episodes it answers with, their Versions' Tracks.
//! It writes only Watch State, for what a reported Version or a marked title stands for (ADR 0013).

use std::{collections::HashMap, sync::Arc};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::{PageRequest, PageResponse, SortDirection, UnmatchedItemSort};

use crate::{
    db::Database,
    library::{
        Contents, Found, Libraries, Library, LibraryEpisode, LibraryError, LibraryKind,
        LibraryMovie, LibrarySeason, LibraryShow, LibraryVersion, Listing, NotFound, OnServer,
        Stored, TitleKind, TitleTarget as Target, Titles, Unmatched, WatchState, WatchStateKind,
        WatchStates,
    },
    page::{self, Page},
    playback::{Probe, StoredTracks, Track},
    plugins::ExtensionPoints,
    scanner::read_only_folder,
};

mod box_sets;
mod featured;
mod people;
mod title_pages;
mod watching;

pub(crate) use box_sets::SeenBoxSet;
pub(crate) use featured::Featured;
pub(crate) use people::{PersonPage, PersonTitle};
pub(crate) use title_pages::{FoundPageError, TitlePages};
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
    pub(crate) edition: Option<String>,
    /// What probing read from the file as it is now: none if it hasn't.
    pub(crate) tracks: Vec<Track>,
}

/// Why Browse found nothing, or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    NotFound(#[from] NotFound),
    #[error(transparent)]
    PageTokenInvalid(#[from] page::TokenInvalid),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// How many of `show`'s Seasons the Library has an Episode of, Specials aside.
pub(crate) fn season_count(show: &Show) -> usize {
    (show.seasons.keys())
        .filter(|&&season| season != watching::SPECIALS)
        .count()
}

/// A Show as a page of a Library's Shows lists it.
#[derive(Debug)]
pub(crate) struct ListedShow {
    pub(crate) show: Show,
    /// How many Episodes the Library has of it.
    pub(crate) episodes: u32,
    /// How many of those the User has played.
    pub(crate) played: u32,
}

/// Reads the Libraries' titles and the Users' Watch States of them, and records Watch State.
#[derive(Debug, Clone)]
pub(crate) struct Browse {
    libraries: Libraries,
    contents: Contents,
    tracks: StoredTracks,
    watch_states: WatchStates,
}

impl Browse {
    /// Browse of what `db` holds, telling the Observers of `points` of each Watch State change.
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints) -> Self {
        Self {
            libraries: Libraries::new(Arc::clone(&db)),
            contents: Contents::new(Arc::clone(&db)),
            tracks: StoredTracks::new(Arc::clone(&db)),
            watch_states: WatchStates::new(db, points),
        }
    }

    /// A page of the Movies of the Movies Library `library` that `listing` keeps, in its order,
    /// each without Tracks and with the User `user`'s Watch State of it, if they've started it.
    pub(crate) async fn movie_page(
        &self,
        user: i64,
        library: i64,
        listing: &Listing,
        asked: &PageRequest,
    ) -> Result<(Vec<(LibraryMovie, Option<WatchState>)>, PageResponse), Error> {
        let (ids, paged) =
            (self.title_page(user, (library, LibraryKind::Movies), listing, asked)).await?;
        let movies = self.contents.movies(library, Some(&ids)).await?;
        let movies = in_order_of(&ids, movies, |movie| &movie.id);
        let titles = movies.iter().map(movie_ids).collect();
        let states = self.states_of(user, titles).await?;
        Ok((movies.into_iter().zip(states).collect(), paged))
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

    /// A page of the Shows of the Shows Library `library` that `listing` keeps, in its order, each
    /// with how many of its Episodes the User `user` has played.
    pub(crate) async fn show_page(
        &self,
        user: i64,
        library: i64,
        listing: &Listing,
        asked: &PageRequest,
    ) -> Result<(Vec<ListedShow>, PageResponse), Error> {
        let (ids, paged) =
            (self.title_page(user, (library, LibraryKind::Shows), listing, asked)).await?;
        let shows = self.contents.shows(library, Some(&ids)).await?;
        let shows = in_order_of(&ids, shows, |show| &show.id);
        let every_episode = shows.iter().flat_map(Show::episodes);
        let states = self
            .states_of(user, every_episode.map(episode_ids).collect())
            .await?;
        let mut states = states.into_iter();
        let listed = shows.into_iter().map(|show| {
            let episodes = show.episodes().count();
            let played = (states.by_ref().take(episodes))
                .filter(|state| state.is_some_and(|state| state.played))
                .count();
            ListedShow {
                show,
                episodes: u32::try_from(episodes).unwrap_or(u32::MAX),
                played: u32::try_from(played).unwrap_or(u32::MAX),
            }
        });
        Ok((listed.collect(), paged))
    }

    /// The genres of the titles of the Library `library`, by name.
    pub(crate) async fn genres(&self, library: i64) -> Result<Vec<String>, Error> {
        let library = self.library(library).await?;
        Ok(self.contents.genres(library.id).await?)
    }

    /// The Show `id` of the Shows Library `library`.
    pub(crate) async fn show(&self, library: i64, id: &ExternalId) -> Result<Show, Error> {
        Ok(self.titles(&self.library(library).await?).await?.show(id)?)
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

    /// The best `limit` Movies, Shows and Episodes of every Library that a search for `query`
    /// finds, of `kind` only if it's set: see [`Contents::search`].
    pub(crate) async fn search(
        &self,
        query: &str,
        kind: Option<TitleKind>,
        limit: usize,
    ) -> Result<Vec<Found>, sqlx::Error> {
        self.contents.search(query, kind, limit).await
    }

    /// The `asked` page of the Unmatched Items of the Library `library`, the one found most
    /// recently first. Its token holds no path, which a User may not see (ADR 0069).
    pub(crate) async fn unmatched(
        &self,
        library: i64,
        asked: &PageRequest,
    ) -> Result<(Vec<Unmatched>, PageResponse), Error> {
        let library = self.library(library).await?;
        let page = Page::read(asked, format!("ListUnmatched {}", library.id).as_bytes())?;
        let newest_first = (
            UnmatchedItemSort::UNMATCHED_ITEM_SORT_FOUND,
            SortDirection::SORT_DIRECTION_DESCENDING,
        );
        Ok(self
            .contents
            .unmatched(library.id, newest_first, &page)
            .await?)
    }

    /// The Version `version` of the Library `library`, with its Tracks and its first Unmatched
    /// Item, for playing it. [`NotFound::NoSuchVersion`] unless it holds one.
    pub(crate) async fn unmatched_version(
        &self,
        library: i64,
        version: i64,
    ) -> Result<(Unmatched, Version), Error> {
        let library = self.library(library).await?;
        let unmatched = (self
            .contents
            .unmatched_in_version(library.id, version)
            .await?)
            .ok_or(NotFound::NoSuchVersion)?;
        let (path, file) =
            (self.contents.version_file(version).await?).ok_or(NotFound::NoSuchVersion)?;
        // An Unmatched Item has this one Version, listed beside no other, so its Edition tells
        // nothing apart.
        let found = LibraryVersion {
            file,
            path,
            edition: None,
        };
        let probes = self.probes(std::iter::once(&found)).await?;
        Ok((unmatched, with_tracks(&probes)(found)))
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

    /// Every title of `library`, without Tracks. Every finder reads through here; one answering
    /// with Tracks reads them afterwards, of only what it answers with.
    async fn titles(&self, library: &Library) -> Result<Titles, Error> {
        Ok(Titles::read(&self.contents, library).await?)
    }

    /// The ids of a page of the titles of the `kind` Library `library` that `listing` keeps, as
    /// [`Contents::title_page`] lists them for the User `user`.
    async fn title_page(
        &self,
        user: i64,
        (library, kind): (i64, LibraryKind),
        listing: &Listing,
        asked: &PageRequest,
    ) -> Result<(Vec<ExternalId>, PageResponse), Error> {
        let library = self.library(library).await?;
        if library.kind != kind {
            return Err(NotFound::WrongKind { wanted: kind }.into());
        }
        let list = format!("{kind:?} of Library {} {listing:?}", library.id);
        let page = Page::read(asked, list.as_bytes())?;
        // Only the watched filter reads them, and a User may have thousands.
        let started = match (listing.watched, kind) {
            (None, _) => Vec::new(),
            (Some(_), LibraryKind::Movies) => {
                self.watch_states
                    .started(user, WatchStateKind::Movie)
                    .await?
            }
            (Some(_), LibraryKind::Shows) => {
                self.watch_states
                    .started(user, WatchStateKind::Episode)
                    .await?
            }
        };
        let page = (self.contents)
            .title_page(library.id, listing, &started, &page)
            .await?;
        Ok(page)
    }

    /// What probing read from the files of `versions` as they are now, with their sidecars, by
    /// file id.
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

    /// The titles of `kind` that `ids` name and a Library has, in the order of `ids`, each once,
    /// in the oldest Library that has it, by any id it shares with one of `ids`. Every Library
    /// counts, since every User may browse each one.
    pub(crate) async fn in_libraries(
        &self,
        kind: TitleKind,
        ids: &[ExternalId],
    ) -> Result<Vec<OnServer>, Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let libraries = self.libraries.list().await.map_err(finding_failed)?;
        let kind = match kind {
            TitleKind::Movie => LibraryKind::Movies,
            TitleKind::Show => LibraryKind::Shows,
        };
        Ok(OnServer::in_order_of(&self.contents, &libraries, kind, ids).await?)
    }

    pub(super) async fn library(&self, library: i64) -> Result<Library, Error> {
        self.libraries.get(library).await.map_err(finding_failed)
    }
}

/// The titles of `titles` named by `ids`, in that order, each title's id as `id_of` reads it. A
/// title gone since `ids` were read is left out.
fn in_order_of<T>(ids: &[ExternalId], titles: Vec<T>, id_of: impl Fn(&T) -> &ExternalId) -> Vec<T> {
    let mut by_id: HashMap<ExternalId, T> = (titles.into_iter())
        .map(|title| (id_of(&title).clone(), title))
        .collect();
    ids.iter().filter_map(|id| by_id.remove(id)).collect()
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
            edition: version.edition,
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
        LibraryError::NotFound(id) => NotFound::NoSuchLibrary(id).into(),
        LibraryError::Database(err) => Error::Database(err),
        LibraryError::NameBlank
        | LibraryError::NameTaken { .. }
        | LibraryError::FoldersEmpty
        | LibraryError::Folder { .. }
        | LibraryError::FolderTaken { .. }
        | LibraryError::FolderOverlaps { .. }
        | LibraryError::TitleSources { .. }
        | LibraryError::TitleSourceUnknown { .. }
        | LibraryError::NoTitleSourceSearches
        | LibraryError::RecognisersInvalid { .. }
        | LibraryError::SegmentSourcesInvalid { .. } => {
            unreachable!("finding a Library refused a change: {err}")
        }
    }
}

#[cfg(test)]
#[path = "browse/browse_tests.rs"]
mod tests;
