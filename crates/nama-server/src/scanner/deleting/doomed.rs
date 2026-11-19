//! What a delete takes: the Versions it names, the Episodes it takes along, what Fulfillers stop
//! monitoring and which Requests it touches.

use std::collections::{BTreeSet, HashSet};

use nama_plugin_api::{
    fulfiller::{
        EpisodeNumber, EpisodeNumbers, MovieDeleted, ShowDeleted, TitleId, show_deleted::Deleted,
        stop_monitoring::Title,
    },
    metadata::{ExternalId, ExternalIds},
};

use super::{TakenAlong, ToDelete, files::read_only_folder};
use crate::{
    library::{
        Library, LibraryEpisode, LibraryError, LibraryShow, LibraryVersion, NotFound,
        TitleTarget as Target, Titles, known_as, named_episodes,
    },
    requests::RequestTitle,
    scanner::{Error, Scanner},
};

/// Versions of one title of a Library, which a delete removes, with the title, for its log.
#[derive(Debug)]
pub(super) struct Doomed {
    pub(super) library: Library,
    pub(super) title: String,
    /// What the Library's Fulfillers are asked to stop monitoring. `None` for a title Matched
    /// without the ids they know it by.
    pub(super) to_unmonitor: Option<Title>,
    /// Each file once.
    pub(super) versions: Vec<LibraryVersion>,
    /// The Episodes the title isn't, that a file of `versions` holds too.
    pub(super) taken_along: Vec<TakenAlong>,
    /// What each Version is of, by its id, as a Request names it: its Movie, or the Season of each
    /// Episode it holds.
    pub(super) requested: Vec<(i64, RequestTitle)>,
}

impl Doomed {
    pub(super) fn version_ids(&self) -> Vec<i64> {
        (self.versions.iter())
            .map(|version| version.file.id)
            .collect()
    }

    /// What the Versions `gone` are of, as Requests name it, each once.
    pub(super) fn request_titles(&self, gone: &[i64]) -> Vec<RequestTitle> {
        let titles = (self.requested.iter()).filter(|(version, _)| gone.contains(version));
        let titles: BTreeSet<RequestTitle> = titles.map(|(_, title)| title.clone()).collect();
        titles.into_iter().collect()
    }

    /// The ids of the Episodes it takes along.
    pub(super) fn taken_along_ids(&self) -> Vec<ExternalId> {
        (self.taken_along.iter())
            .map(|taken| taken.episode.id.clone())
            .collect()
    }

    /// The folder of the Library holding one of the Versions that the Library's last scan found
    /// the Server can't write to, which refuses the delete.
    pub(super) fn read_only_folder(&self) -> Option<&str> {
        read_only_folder(&self.library, self.versions.iter().map(|v| v.path.as_str()))
    }

    /// Refuses the delete, in this order, for a Version in a read-only folder
    /// ([`Error::ReadOnly`]), or for an Episode taken along that none of `also` names
    /// ([`Error::TakesAlongUnnamed`]), naming each such Episode.
    pub(super) fn check(&self, also: &[ExternalId]) -> Result<(), Error> {
        if let Some(folder) = self.read_only_folder() {
            return Err(Error::ReadOnly(folder.to_owned()));
        }
        let named = |episode: &LibraryEpisode| {
            (also.iter()).any(|id| &episode.id == id || known_as(&episode.ids, id))
        };
        let unnamed: Vec<TakenAlong> = (self.taken_along.iter())
            .filter(|taken| !named(&taken.episode))
            .cloned()
            .collect();
        if unnamed.is_empty() {
            return Ok(());
        }
        Err(Error::TakesAlongUnnamed(unnamed))
    }
}

impl Scanner {
    /// What deleting `target` takes, each file once. Fails with [`NotFound`] as browsing to the
    /// title does, or with [`NotFound::NoSuchVersion`] for a Version of no Movie, or not of the
    /// Episode named.
    pub(super) async fn doomed(&self, target: &ToDelete) -> Result<Doomed, Error> {
        let (library, title, only) = match target {
            &ToDelete::Version(version) => return self.doomed_version(version).await,
            ToDelete::Title { library, title } => (*library, title.clone(), None),
            ToDelete::EpisodeVersion {
                library,
                episode,
                version,
            } => (*library, Target::Episode(episode.clone()), Some(*version)),
        };
        let library = self.library_to_delete_from(library).await?;
        let titles = Titles::read(&self.inner.contents, &library).await?;
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
        Ok(doomed_episodes(library, &titles.shows()?, &title, only)?)
    }

    /// The Version `version` of a Movie, with its Library and the Movie's title.
    async fn doomed_version(&self, version: i64) -> Result<Doomed, Error> {
        let library = self.inner.contents.library_of_version(version).await?;
        let library =
            (self.library_to_delete_from(library.ok_or(NotFound::NoSuchVersion)?)).await?;
        let Titles::Movies(movies) = Titles::read(&self.inner.contents, &library).await? else {
            return Err(NotFound::NoSuchVersion.into());
        };
        let (title, id, to_unmonitor, version) = (movies.into_iter())
            .find_map(|movie| {
                let version = movie.versions.into_iter().find(|v| v.file.id == version)?;
                let to_unmonitor = movie_to_unmonitor(&movie.ids);
                Some((movie.title, movie.id, to_unmonitor, version))
            })
            .ok_or(NotFound::NoSuchVersion)?;
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

    async fn library_to_delete_from(&self, id: i64) -> Result<Library, Error> {
        match self.inner.libraries.get(id).await {
            Err(LibraryError::NotFound(id)) => Err(NotFound::NoSuchLibrary(id).into()),
            Err(LibraryError::Database(err)) => Err(err.into()),
            found => Ok(found?),
        }
    }
}

/// The Versions of `shows`, a Shows Library's, that `target` names, or only its Version `only`,
/// each file once, with the other Episodes those files hold.
fn doomed_episodes(
    library: Library,
    shows: &[LibraryShow],
    target: &Target,
    only: Option<i64>,
) -> Result<Doomed, NotFound> {
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
        return Err(NotFound::NoSuchVersion);
    }
    let files: HashSet<i64> = versions.iter().map(|version| version.file.id).collect();
    let holds =
        |episode: &LibraryEpisode| (episode.versions.iter()).any(|v| files.contains(&v.file.id));
    let taken_along: Vec<TakenAlong> = (shows.iter())
        .flat_map(|holder| holder.episodes().map(move |episode| (holder, episode)))
        .filter(|(_, episode)| !named.iter().any(|named| named.id == episode.id))
        .filter(|(_, episode)| holds(episode))
        .map(|(holder, episode)| TakenAlong {
            show: holder.title.clone(),
            episode: episode.clone(),
        })
        .collect();
    let episodes = (named.into_iter()).chain(taken_along.iter().map(|taken| &taken.episode));
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
