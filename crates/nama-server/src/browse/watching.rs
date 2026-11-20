//! Watch State through Browse: what a User reports, marks and has started.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::{PageRequest, PageResponse};

use super::{
    Browse, Episode, Error, NotFound, Show, Target, Titles, Version, episode_ids, finding_failed,
    with_tracks,
};
use crate::{
    library::{
        LibraryEpisode, LibraryMovie, LibraryVersion, MovieOrEpisode, Progress, TitlesIn,
        WatchState, WatchedTitle,
    },
    page::Page,
    playback::Probe,
};

/// A Movie or Episode in a User's Continue Watching, in the Library it lists it under, with
/// Versions of the form `V`.
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
        /// `None` for the Show's next Episode, which the User hasn't started.
        state: Option<WatchState>,
    },
}

/// Where a title sits in Continue Watching, newest first: when its latest Watch State, or its
/// Show's latest of any Episode, changed, and that state's row id, which breaks ties within a
/// second.
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
            TitlesIn::NoSuchVersion => return Err(NotFound::NoSuchVersion.into()),
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
    /// preferred id; unplayed forgets it under every id. Either, where it changes a Watch State,
    /// shows the Movie or Show again in Continue Watching. Fails as browsing to it does if the
    /// Library has no such title.
    pub(crate) async fn mark(
        &self,
        user: i64,
        library: i64,
        target: Target,
        played: bool,
    ) -> Result<(), Error> {
        let Targeted { titles, entry } = self.targeted(library, target).await?;
        if played {
            Ok(self.watch_states.mark(user, titles, true).await?)
        } else {
            Ok(self.watch_states.mark_unplayed(user, titles, entry).await?)
        }
    }

    /// Hides the Movie or Show of `target` of the Library `library` from the User `user`'s
    /// Continue Watching, or shows it again, without changing its Watch State. It stays hidden
    /// until the User's Watch State of the Movie, or of any of the Show's Episodes, next changes.
    /// Fails as browsing to it does if the Library has no such title.
    pub(crate) async fn hide(
        &self,
        user: i64,
        library: i64,
        target: Target,
        hidden: bool,
    ) -> Result<(), Error> {
        let Targeted { entry, .. } = self.targeted(library, target).await?;
        Ok(self.watch_states.hide(user, entry, hidden).await?)
    }

    /// What `target` of the Library `library` stands for.
    async fn targeted(&self, library: i64, target: Target) -> Result<Targeted, Error> {
        let titles = self.titles(&self.library(library).await?).await?;
        let episode =
            |episode: &LibraryEpisode| MovieOrEpisode::episode(&episode.id).known_as(&episode.ids);
        let every_episode = |show: &Show| show.episodes().map(episode).collect();
        Ok(match target {
            Target::Movie(id) => {
                let movie = titles.movie(&id)?;
                Targeted::entry(vec![MovieOrEpisode::movie(&movie.id).known_as(&movie.ids)])
            }
            Target::Episode(id) => {
                let (show, marked) = titles.episode(&id)?;
                Targeted {
                    titles: vec![episode(&marked)],
                    entry: every_episode(&show),
                }
            }
            Target::Season { show, number } => {
                let show = titles.show(&show)?;
                let season = show.seasons.get(&number).ok_or(NotFound::NoSuchSeason)?;
                Targeted {
                    titles: season.episodes.iter().map(episode).collect(),
                    entry: every_episode(&show),
                }
            }
            Target::Show(id) => Targeted::entry(every_episode(&titles.show(&id)?)),
        })
    }

    /// A page of the User `user`'s Continue Watching, most recent first: the Movies they've
    /// started and not played, and one Episode of each Show they've watched, the one they started
    /// most recently if they haven't played it, or else the Show's next Episode. Each is in the
    /// oldest Library that has it under any of its ids, with its most recently updated Watch State
    /// among them, so a newer played one under one id hides an older started one under another
    /// (ADR 0042). One no Library has is left out. Only the first page counts them all.
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

    /// The Episode that plays after Episode `number` of Season `season` of the Show `show` of the
    /// Shows Library `library`, with the User `user`'s Watch State of it: the first of the Show's
    /// Episodes in the Library after it that they haven't played (see [`unplayed_after`]). Fails as
    /// browsing to that Season does, or with [`NotFound::NoSuchEpisode`].
    pub(crate) async fn episode_after(
        &self,
        user: i64,
        library: i64,
        show: &ExternalId,
        (season, number): (i64, i64),
    ) -> Result<Option<(Episode, Option<WatchState>)>, Error> {
        let show = self.show(library, show).await?;
        let season_episodes = &show
            .seasons
            .get(&season)
            .ok_or(NotFound::NoSuchSeason)?
            .episodes;
        let from = (season_episodes.iter())
            .find(|episode| episode.number == number)
            .ok_or(NotFound::NoSuchEpisode)?;
        let later: Vec<_> = (show.episodes())
            .filter(|episode| (episode.season, episode.number) > (season, number))
            .collect();
        let titles = later.iter().map(|episode| episode_ids(episode)).collect();
        let states = self.states_of(user, titles).await?;
        let Some((next, state)) = unplayed_after(from, later.into_iter().zip(states)) else {
            return Ok(None);
        };
        let probes = self.probes(next.versions.iter()).await?;
        Ok(Some((
            next.clone().map_versions(with_tracks(&probes)),
            state,
        )))
    }

    /// The whole of [`Self::continue_watching`], without Tracks, each title with its recency.
    pub(super) async fn all_started(
        &self,
        user: i64,
    ) -> Result<Vec<(Started<LibraryVersion>, Recency)>, Error> {
        let states = self.watch_states.of_user(user).await?;
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let hidden: HashSet<i64> = (states.iter())
            .filter_map(|&(_, _, row, hidden)| hidden.then_some(row))
            .collect();
        let mut found = Found {
            unfound: (states.into_iter())
                .map(|(watched, state, row, _)| (watched, (state, row)))
                .collect(),
            listed: Vec::new(),
        };
        // Oldest first, so a title in two Libraries is found in the older.
        for library in self.libraries.list().await.map_err(finding_failed)? {
            match self.titles(&library).await? {
                Titles::Movies(movies) => found.movies(library.id, movies),
                Titles::Shows(shows) => found.shows(library.id, shows),
            }
        }
        // Hidden while the Watch State that dates it is: a newer change to any of a Show's
        // Episodes dates it instead.
        (found.listed).retain(|(_, (_, row))| !hidden.contains(row));
        found.listed.sort_by_key(|&(_, recency)| Reverse(recency));
        Ok(found.listed)
    }
}

impl Started<LibraryVersion> {
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

/// The Movie or Episodes a [`Target`] stands for, each by every id, and the whole Movie or Show
/// they are of, as its Continue Watching entry: a Show is every one of its Episodes.
struct Targeted {
    titles: Vec<WatchedTitle>,
    entry: Vec<WatchedTitle>,
}

impl Targeted {
    /// A [`Target`] that stands for the whole of its entry, as a Movie or Show does.
    fn entry(titles: Vec<WatchedTitle>) -> Self {
        Self {
            entry: titles.clone(),
            titles,
        }
    }
}

/// What of the Libraries read so far is in a User's Continue Watching.
struct Found {
    /// The User's Watch States, each with its row id, under ids no Library read so far has.
    unfound: HashMap<MovieOrEpisode, (WatchState, i64)>,
    /// Each title listed, with its recency.
    listed: Vec<(Started<LibraryVersion>, Recency)>,
}

impl Found {
    /// Takes the User's Watch States under any of `ids` as one title's, and returns the latest.
    // ponytail: a title in two Libraries with different id sets, watched both under an id the
    // older one has and under one only the newer one has, is listed twice: the older Library's
    // copy takes the ids it has and the newer one's takes the rest. Joining a title whose ids
    // include one already taken to that title would list it once.
    fn take(
        &mut self,
        ids: &ExternalIds,
        watched: impl Fn(&ExternalId) -> MovieOrEpisode,
    ) -> Option<(WatchState, i64)> {
        (ids.iter())
            .filter_map(|id| self.unfound.remove(&watched(&id)))
            .max_by_key(|&(state, row)| (state.updated_at, row))
    }

    /// The `movies` of the Library `library`.
    fn movies(&mut self, library: i64, movies: Vec<LibraryMovie>) {
        for movie in movies {
            let Some((state, row)) = self.take(&movie.ids, MovieOrEpisode::movie) else {
                continue;
            };
            if is_started(state) {
                let started = Started::Movie {
                    library,
                    movie,
                    state,
                };
                self.listed.push((started, (state.updated_at, row)));
            }
        }
    }

    /// The `shows` of the Library `library`, each listed at its latest Watch State.
    // ponytail: a Show whose Episodes are split across Libraries is judged in each on that
    // Library's Episodes alone, so it can be listed twice, or not at all when its next Episode is
    // only in a newer Library. Merge a Show's Episodes across Libraries if that comes up.
    fn shows(&mut self, library: i64, shows: Vec<Show>) {
        for show in shows {
            let watched: Vec<_> = (show.episodes())
                .map(|episode| (episode, self.take(&episode.ids, MovieOrEpisode::episode)))
                .collect();
            let recency = (watched.iter())
                .filter_map(|&(_, taken)| taken.map(|(state, row)| (state.updated_at, row)))
                .max();
            let Some((recency, (episode, state))) = recency.zip(show_entry(&watched)) else {
                continue;
            };
            let episode = episode.clone();
            let started = Started::Episode {
                library,
                show,
                episode,
                state,
            };
            self.listed.push((started, recency));
        }
    }
}

/// Whether a User has started the title of `state` and not played it.
const fn is_started(state: WatchState) -> bool {
    state.position_ms > 0 && !state.played
}

/// An Episode, with the User's latest Watch State of it and that state's row id.
pub(super) type WatchedEpisode<'a, V> = (&'a LibraryEpisode<V>, Option<(WatchState, i64)>);

/// A Show's entry in Continue Watching, from each of its Episodes in aired order: its most
/// recently started Episode, with its state, or else its next Episode, without one. `None` if it
/// has neither.
pub(super) fn show_entry<'a, V>(
    watched: &[WatchedEpisode<'a, V>],
) -> Option<(&'a LibraryEpisode<V>, Option<WatchState>)> {
    let started = (watched.iter())
        .filter_map(|&(episode, taken)| Some((episode, taken?)))
        .filter(|&(_, (state, _))| is_started(state))
        .max_by_key(|&(_, (state, row))| (state.updated_at, row));
    match started {
        Some((episode, (state, _))) => Some((episode, Some(state))),
        None => next_episode(watched).map(|episode| (episode, None)),
    }
}

/// Season 0, which isn't in aired order.
pub(super) const SPECIALS: i64 = 0;

/// The first unplayed Episode of `watched`, in aired order, after the one that became played
/// most recently (see [`unplayed_after`]).
fn next_episode<'a, V>(watched: &[WatchedEpisode<'a, V>]) -> Option<&'a LibraryEpisode<V>> {
    let played_at = |taken: Option<(WatchState, i64)>| taken?.0.played_at;
    // Of those played in the same second, as when a Season is marked played, the last aired.
    let (_, last_played) = (watched.iter().enumerate())
        .filter(|&(_, &(episode, _))| is_aired(episode))
        .filter_map(|(nth, &(_, taken))| Some((played_at(taken)?, nth)))
        .max()?;
    let (from, _) = watched.get(last_played)?;
    let later = (watched.get(last_played + 1..)?.iter())
        .map(|&(episode, taken)| (episode, taken.map(|(state, _)| state)));
    unplayed_after(from, later).map(|(episode, _)| episode)
}

/// The Episode that plays after `from`: the first of the `later` Episodes of its Show, in aired
/// order, each with the User's Watch State of it, that the User hasn't played. `None` after a
/// Special, which has no place in aired order.
fn unplayed_after<'a, V>(
    from: &LibraryEpisode<V>,
    later: impl IntoIterator<Item = (&'a LibraryEpisode<V>, Option<WatchState>)>,
) -> Option<(&'a LibraryEpisode<V>, Option<WatchState>)> {
    if !is_aired(from) {
        return None;
    }
    // No Special is later than an aired Episode: Season 0 sorts before every other Season.
    (later.into_iter()).find(|&(_, state)| state.is_none_or(|state| state.played_at.is_none()))
}

/// Whether `episode` is in aired order, which Specials aren't.
pub(super) fn is_aired<V>(episode: &LibraryEpisode<V>) -> bool {
    episode.season != SPECIALS
}

#[cfg(test)]
#[path = "watching_tests.rs"]
mod tests;
