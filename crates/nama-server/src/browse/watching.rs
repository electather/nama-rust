//! Watch State through Browse: what a User reports, marks and has started.

use std::collections::HashMap;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::{PageRequest, PageResponse};

use super::{Browse, Error, Show, Target, Titles, Version, finding_failed, with_tracks};
use crate::{
    library::{
        LibraryEpisode, LibraryMovie, LibraryVersion, MovieOrEpisode, Progress, TitlesIn,
        WatchState, WatchStateKind,
    },
    page::Page,
    playback::Probe,
};

/// A Movie or Episode a User has started and not played, in the Library Continue Watching lists
/// it under, with Versions of the form `V`.
#[derive(Debug)]
pub(crate) enum Started<V = Version> {
    Movie {
        library: i64,
        movie: LibraryMovie<V>,
        state: WatchState,
    },
    Episode {
        library: i64,
        show: Show,
        episode: LibraryEpisode<V>,
        state: WatchState,
    },
}

/// Where a title sits in Continue Watching, newest first: when the Watch State it's listed by
/// changed, and that state's row id, which breaks ties within a second.
type Recency = (i64, i64);

impl Browse {
    /// Records, as of now, that the User `user` has got as far as `progress` through the Version
    /// `version`: through its Movie, or split across each Episode its file holds, under each one's
    /// preferred id. An Unmatched Item has no Watch State, so progress on it stores nothing (ADR
    /// 0013).
    pub(crate) async fn report(
        &self,
        user: i64,
        version: i64,
        progress: Progress,
    ) -> Result<(), Error> {
        let titles = match self.contents.titles_in(version).await? {
            TitlesIn::NoSuchVersion => return Err(Error::NoSuchVersion),
            TitlesIn::Movie(None) => return Ok(()),
            TitlesIn::Movie(Some((id, ids))) => {
                vec![Some(MovieOrEpisode::movie(&id).known_as(&ids))]
            }
            // An Item that isn't Matched keeps its share of the file, so the others' shares don't
            // move.
            TitlesIn::Episodes(episodes) => (episodes.into_iter())
                .map(|episode| episode.map(|(id, ids)| MovieOrEpisode::episode(&id).known_as(&ids)))
                .collect(),
        };
        Ok(self.watch_states.report(user, titles, progress).await?)
    }

    /// Marks, as of now, `target` of the Library `library` played or unplayed for the User
    /// `user`: a Season or Show is each of its Episodes. Played is written under each one's
    /// preferred id; unplayed forgets it under every id. Fails as browsing to it does if the
    /// Library has no such title.
    pub(crate) async fn mark(
        &self,
        user: i64,
        library: i64,
        target: Target,
        played: bool,
    ) -> Result<(), Error> {
        let titles = self.titles(&self.library(library).await?).await?;
        let episode =
            |episode: &LibraryEpisode| MovieOrEpisode::episode(&episode.id).known_as(&episode.ids);
        let marked = match target {
            Target::Movie(id) => {
                let movie = titles.movie(&id)?;
                vec![MovieOrEpisode::movie(&movie.id).known_as(&movie.ids)]
            }
            Target::Episode(id) => vec![episode(&titles.episode(&id)?)],
            Target::Season { show, number } => (titles.season(&show, number)?.episodes.iter())
                .map(episode)
                .collect(),
            Target::Show(id) => titles.show(&id)?.episodes().map(episode).collect(),
        };
        Ok(self.watch_states.mark(user, marked, played).await?)
    }

    /// A page of the User `user`'s Continue Watching: the Movies and Episodes they've started and
    /// not played, most recent first. Each is in the oldest Library that has it under any of its
    /// ids, listed once with its most recently updated Watch State among them, so a newer played
    /// one under one id hides an older started one under another (ADR 0042). One no Library has
    /// is left out. Only the first page counts them all.
    pub(crate) async fn continue_watching(
        &self,
        user: i64,
        asked: &PageRequest,
    ) -> Result<(Vec<Started>, PageResponse), Error> {
        let page = Page::<i64>::read(asked, b"ListContinueWatching")?;
        // ponytail: each page builds the whole list, which its whole-Library reads already cost;
        // keyset `watch_states` in SQL once Continue Watching stops reading whole Libraries.
        let all = self.all_started(user).await?;
        let total = all.len();
        let mut listed: Vec<_> = (all.into_iter())
            .filter(|&(_, recency)| page.after.is_none_or(|after| recency < after))
            .collect();
        let mut paging = page.finish(&mut listed, |&(_, recency)| recency);
        if page.is_first() {
            paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        let probes = self
            .probes(listed.iter().flat_map(|(title, _)| title.versions()))
            .await?;
        let listed = (listed.into_iter())
            .map(|(title, _)| title.with_tracks(&probes))
            .collect();
        Ok((listed, paging))
    }

    /// The whole of [`Self::continue_watching`], without Tracks, each title with its recency.
    async fn all_started(
        &self,
        user: i64,
    ) -> Result<Vec<(Started<LibraryVersion>, Recency)>, Error> {
        let started = self.watch_states.continue_watching(user).await?;
        if started.is_empty() {
            return Ok(Vec::new());
        }
        let mut found = Found {
            unfound: (started.iter())
                .map(|(watched, state, _)| (watched.clone(), *state))
                .collect(),
            place: HashMap::new(),
            titles: Vec::new(),
        };
        // Oldest first, so a title in two Libraries is found in the older.
        for library in self.libraries.list().await.map_err(finding_failed)? {
            match self.titles(&library).await? {
                Titles::Movies(movies) => found.movies(library.id, movies),
                Titles::Shows(shows) => found.episodes(library.id, &shows),
            }
        }
        let (asked, mut titles): (Vec<_>, Vec<_>) = (found.titles.into_iter())
            .map(|(asked, title)| (asked, Some(title)))
            .unzip();
        let newest = self.watch_states.of_titles(Some(user), &asked).await?;
        Ok((started.into_iter())
            .filter_map(|(watched, WatchState { updated_at, .. }, row)| {
                let place = *found.place.get(&watched)?;
                // Taken under the first of its ids listed, so a title is listed once.
                let title = titles.get_mut(place)?.take()?;
                let &(_, state) = newest.get(place)?.first()?;
                (state.position_ms > 0 && !state.played)
                    .then(|| (title.with_state(state), (updated_at, row)))
            })
            .collect())
    }
}

impl Started<LibraryVersion> {
    /// The same title with the Watch State `state`.
    fn with_state(mut self, state: WatchState) -> Self {
        match &mut self {
            Self::Movie { state: had, .. } | Self::Episode { state: had, .. } => *had = state,
        }
        self
    }

    fn versions(&self) -> &[LibraryVersion] {
        match self {
            Self::Movie { movie, .. } => &movie.versions,
            Self::Episode { episode, .. } => &episode.versions,
        }
    }

    /// The same title, its Versions with the Tracks `probes` has of their files.
    fn with_tracks(self, probes: &HashMap<i64, Probe>) -> Started {
        match self {
            Self::Movie {
                library,
                movie,
                state,
            } => Started::Movie {
                library,
                movie: movie.map_versions(with_tracks(probes)),
                state,
            },
            Self::Episode {
                library,
                show,
                episode,
                state,
            } => Started::Episode {
                library,
                show,
                episode: episode.map_versions(with_tracks(probes)),
                state,
            },
        }
    }
}

/// The titles of the Libraries read so far that a User has started under any of their ids.
struct Found {
    /// The User's started Watch States under ids no Library read so far has.
    unfound: HashMap<MovieOrEpisode, WatchState>,
    /// Which of `titles` each id the User started a title under is of.
    place: HashMap<MovieOrEpisode, usize>,
    /// Each title found, with every id of it, and one of the Watch States it was started with.
    titles: Vec<((WatchStateKind, ExternalIds), Started<LibraryVersion>)>,
}

impl Found {
    /// Adds `title(state)`, a title of kind `kind` with the ids `ids`, if the User started it
    /// under any of them, with `state` one they started it with, and takes its ids as its.
    fn add(
        &mut self,
        kind: WatchStateKind,
        ids: &ExternalIds,
        watched: impl Fn(&ExternalId) -> MovieOrEpisode,
        title: impl FnOnce(WatchState) -> Started<LibraryVersion>,
    ) {
        // ponytail: a title in two Libraries with different id sets, started both under an id
        // the older one has and under one only the newer one has, is listed twice: the older
        // Library's copy takes the ids it has and the newer one's takes the rest. Joining a
        // title whose ids include one already placed to that place would list it once.
        let place = self.titles.len();
        let mut started = None;
        for id in ids.iter() {
            let watched = watched(&id);
            if let Some(state) = self.unfound.remove(&watched) {
                self.place.insert(watched, place);
                started = started.or(Some(state));
            }
        }
        if let Some(state) = started {
            self.titles.push(((kind, ids.clone()), title(state)));
        }
    }

    /// The `movies` of the Library `library`.
    fn movies(&mut self, library: i64, movies: Vec<LibraryMovie>) {
        for movie in movies {
            let ids = movie.ids.clone();
            let started = |state| Started::Movie {
                library,
                movie,
                state,
            };
            self.add(WatchStateKind::Movie, &ids, MovieOrEpisode::movie, started);
        }
    }

    /// The Episodes of `shows`, of the Library `library`.
    fn episodes(&mut self, library: i64, shows: &[Show]) {
        for show in shows {
            for episode in show.episodes() {
                // ponytail: a Show cloned per started Episode; `Arc<Show>` if a profile ever
                // points here.
                let started = |state| Started::Episode {
                    library,
                    show: show.clone(),
                    episode: episode.clone(),
                    state,
                };
                let kind = WatchStateKind::Episode;
                self.add(kind, &episode.ids, MovieOrEpisode::episode, started);
            }
        }
    }
}
