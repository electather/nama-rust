//! What a delete takes: the Versions a User's delete names, and what Fulfillers stop monitoring.

use std::collections::HashSet;

use nama_plugin_api::{
    fulfiller::{
        EpisodeNumber, EpisodeNumbers, MovieDeleted, ShowDeleted, TitleId, show_deleted::Deleted,
        stop_monitoring::Title,
    },
    metadata::{ExternalId, ExternalIds},
};

use super::{Browse, Error, Target, Titles};
use crate::{
    library::{Library, LibraryEpisode, LibraryKind, LibraryShow, LibraryVersion},
    requests::RequestTitle,
    scanner::Doomed,
};

/// What a User deletes: see [`Browse::doomed`].
#[derive(Debug)]
pub(crate) enum ToDelete {
    /// Every Version of a title of a Library: a Movie, or a Show's every Episode, a Season's or
    /// one Episode.
    Title { library: i64, title: Target },
    /// One Version of a Movie.
    Version(i64),
    /// One Version of an Episode of a Shows Library.
    EpisodeVersion {
        library: i64,
        episode: ExternalId,
        version: i64,
    },
}

impl Browse {
    /// The Versions `target` names, each file once, with their Library, their title and the
    /// other Episodes those files hold. Fails as browsing to the title does, or with
    /// [`Error::NoSuchVersion`] for a Version of no Movie, or not of the Episode named.
    pub(crate) async fn doomed(&self, target: ToDelete) -> Result<Doomed, Error> {
        let (library, title, only) = match target {
            ToDelete::Version(version) => return self.doomed_version(version).await,
            ToDelete::Title { library, title } => (library, title, None),
            ToDelete::EpisodeVersion {
                library,
                episode,
                version,
            } => (library, Target::Episode(episode), Some(version)),
        };
        let library = self.library(library).await?;
        let titles = self.titles(&library).await?;
        if let Target::Movie(id) = title {
            let movie = titles.movie(&id)?;
            return Ok(Doomed {
                library,
                to_unmonitor: movie_to_unmonitor(&movie.ids),
                title: movie.title,
                requested: movie_requested(&movie.id, &movie.versions),
                versions: movie.versions,
                taken_along: Vec::new(),
            });
        }
        doomed_episodes(library, &titles.shows()?, &title, only)
    }

    /// The Version `version` of a Movie, with its Library and the Movie's title.
    async fn doomed_version(&self, version: i64) -> Result<Doomed, Error> {
        let library = self.contents.library_of_version(version).await?;
        let library = self.library(library.ok_or(Error::NoSuchVersion)?).await?;
        let Titles::Movies(movies) = self.titles(&library).await? else {
            return Err(Error::NoSuchVersion);
        };
        let (title, id, to_unmonitor, version) = (movies.into_iter())
            .find_map(|movie| {
                let version = movie.versions.into_iter().find(|v| v.file.id == version)?;
                let to_unmonitor = movie_to_unmonitor(&movie.ids);
                Some((movie.title, movie.id, to_unmonitor, version))
            })
            .ok_or(Error::NoSuchVersion)?;
        let versions = vec![version];
        Ok(Doomed {
            library,
            to_unmonitor,
            title,
            requested: movie_requested(&id, &versions),
            versions,
            taken_along: Vec::new(),
        })
    }
}

/// The Versions of `shows`, a Shows Library's, that `target` names, or only its Version `only`,
/// each file once, with the other Episodes those files hold.
fn doomed_episodes(
    library: Library,
    shows: &[LibraryShow],
    target: &Target,
    only: Option<i64>,
) -> Result<Doomed, Error> {
    let (title, show, named) = named_episodes(shows, target)?;
    let mut versions: Vec<LibraryVersion> = (named.iter())
        .flat_map(|episode| &episode.versions)
        .filter(|version| only.is_none_or(|only| version.file.id == only))
        .cloned()
        .collect();
    // A file holding two Episodes is a Version of each, and goes once.
    versions.sort_unstable_by_key(|version| version.file.id);
    versions.dedup_by_key(|version| version.file.id);
    if versions.is_empty() {
        return Err(Error::NoSuchVersion);
    }
    let files: HashSet<i64> = versions.iter().map(|version| version.file.id).collect();
    let taken_along: Vec<LibraryEpisode> = (shows.iter().flat_map(LibraryShow::episodes))
        .filter(|episode| !named.iter().any(|named| named.id == episode.id))
        .filter(|episode| (episode.versions.iter()).any(|v| files.contains(&v.file.id)))
        .cloned()
        .collect();
    let episodes = named.into_iter().chain(&taken_along);
    Ok(Doomed {
        library,
        to_unmonitor: to_unmonitor_of(show, target, episodes),
        title,
        requested: seasons_requested(shows, &files),
        versions,
        taken_along,
    })
}

/// What a delete of a Movie with `ids` asks Fulfillers to stop monitoring. `None` without ids.
fn movie_to_unmonitor(ids: &ExternalIds) -> Option<Title> {
    let ids = title_ids(ids);
    (!ids.is_empty()).then(|| {
        Title::from(MovieDeleted {
            ids,
            ..Default::default()
        })
    })
}

/// `ids` as the Plugin contract carries a title's ids.
fn title_ids(ids: &ExternalIds) -> Vec<TitleId> {
    (ids.iter())
        .map(|id| TitleId {
            namespace: id.namespace().to_owned(),
            value: id.value().to_owned(),
            ..Default::default()
        })
        .collect()
}

/// Each of `versions` with the Movie `id` it is of, as a Request names it.
fn movie_requested(id: &ExternalId, versions: &[LibraryVersion]) -> Vec<(i64, RequestTitle)> {
    let title = RequestTitle {
        id: id.clone(),
        season: None,
    };
    (versions.iter())
        .map(|version| (version.file.id, title.clone()))
        .collect()
}

/// Each of `files` with the Season of each Episode of `shows` it holds, as a Request names it.
fn seasons_requested(shows: &[LibraryShow], files: &HashSet<i64>) -> Vec<(i64, RequestTitle)> {
    let mut requested = Vec::new();
    for show in shows {
        for episode in show.episodes() {
            let Ok(season) = u32::try_from(episode.season) else {
                continue;
            };
            let title = RequestTitle {
                id: show.id.clone(),
                season: Some(season),
            };
            let held = (episode.versions.iter()).filter(|version| files.contains(&version.file.id));
            requested.extend(held.map(|version| (version.file.id, title.clone())));
        }
    }
    requested
}

/// What a delete of `target`, Episodes of `show`, asks Fulfillers to stop monitoring: the
/// Episodes `episodes`, a whole Season or every current Season. `None` if the Show has no ids.
fn to_unmonitor_of<'a>(
    show: &LibraryShow,
    target: &Target,
    episodes: impl Iterator<Item = &'a LibraryEpisode>,
) -> Option<Title> {
    let show_ids = title_ids(&show.ids);
    if show_ids.is_empty() {
        return None;
    }
    // A scan parses season and episode numbers from digits, so they are never negative.
    let number = |n: i64| u32::try_from(n).unwrap_or_default();
    let deleted = match target {
        Target::Show(_) => Deleted::EveryCurrentSeason(true),
        // Its files take along only its own Episodes: a scan reads one Season per file name.
        Target::Season { number: season, .. } => Deleted::SeasonNumber(number(*season)),
        // A Movie never gets here: `named_episodes` refused it.
        Target::Episode(_) | Target::Movie(_) => {
            let episodes = (episodes.map(|episode| EpisodeNumber {
                season_number: number(episode.season),
                episode_number: number(episode.number),
                ..Default::default()
            }))
            .collect();
            Deleted::from(EpisodeNumbers {
                episodes,
                ..Default::default()
            })
        }
    };
    Some(Title::from(ShowDeleted {
        show_ids,
        deleted: Some(deleted),
        ..Default::default()
    }))
}

/// Whether `id` is one of `ids`: any one identifies a title (ADR 0042).
pub(super) fn names(ids: &ExternalIds, id: &ExternalId) -> bool {
    ids.get(id.namespace()).as_ref() == Some(id)
}

/// The Episodes of `shows` that `target` names, with its name for the log and their Show. A Movie
/// is in no Shows Library.
fn named_episodes<'a>(
    shows: &'a [LibraryShow],
    target: &Target,
) -> Result<(String, &'a LibraryShow, Vec<&'a LibraryEpisode>), Error> {
    let show_of = |id: &ExternalId| {
        (shows.iter().find(|show| names(&show.ids, id))).ok_or(Error::NothingMatched)
    };
    match target {
        Target::Movie(_) => Err(Error::WrongKind {
            wanted: LibraryKind::Movies,
        }),
        Target::Show(id) => {
            let show = show_of(id)?;
            Ok((show.title.clone(), show, show.episodes().collect()))
        }
        Target::Season { show, number } => {
            let show = show_of(show)?;
            let season = show.seasons.get(number).ok_or(Error::NoSuchSeason)?;
            let name = format!("{} Season {number}", show.title);
            Ok((name, show, season.episodes.iter().collect()))
        }
        Target::Episode(id) => (shows.iter())
            .find_map(|show| {
                let episode = show.episodes().find(|episode| &episode.id == id)?;
                let name = format!(
                    "{} S{:02}E{:02}",
                    show.title, episode.season, episode.number
                );
                Some((name, show, vec![episode]))
            })
            .ok_or(Error::NothingMatched),
    }
}
