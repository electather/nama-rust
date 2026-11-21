//! Looking titles up in the title source for Requests, as a viewer sees them.

use std::collections::HashSet;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, TitleMethod},
};
use nama_proto::nama::v1::{
    FollowedShow, FoundSeason, Permission, Request, RequestState, RequestedQuality,
};
use sqlx::SqliteConnection;

use super::asking::kept_under;
use super::availability::{availability, today};
use super::followed::{load_followed, open_followed_show, open_followed_show_by_any};
use super::quality::show_requests;
use super::{Error, Requests};
use crate::{
    library::{
        Candidate, ImageRef, Metadata, SearchQuery, Source, SourceSeason, SourceShow, TitleKind,
        Unanswered, library_with_movie, library_with_show, match_bands, matched_episodes,
    },
    users::User,
};

/// A Movie the title source has, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct Found<M> {
    /// The id its Request is kept under.
    pub(crate) id: ExternalId,
    /// The Plugin whose title source gave the Movie, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) movie: M,
    /// The oldest Library with the Movie Matched, if any.
    pub(crate) library: Option<i64>,
    /// The Movie's open Request, if it has one the viewer may see.
    pub(crate) request: Option<Request>,
}

/// A Show the title source has, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct ShowToRequest {
    pub(crate) hit: Hit,
    /// The oldest Library with an Episode of it Matched, if any.
    pub(crate) library: Option<i64>,
    /// The open Request for its earliest Season with one the viewer may see, if any.
    pub(crate) request: Option<Request>,
    /// Whether it has an open Followed Show the viewer may see.
    pub(crate) followed: bool,
}

/// A title a search found, which a Request can name by its id.
#[derive(Debug)]
pub(crate) struct Hit {
    pub(crate) id: ExternalId,
    /// The Plugin whose title source found it, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) candidate: Candidate,
}

/// A title the title source has, with what a list of titles shows of it.
#[derive(Debug)]
pub(crate) struct Listed {
    /// The id its Request is kept under.
    pub(crate) id: ExternalId,
    pub(crate) title: String,
    /// The year a Movie was released or a Show first aired.
    pub(crate) year: Option<i32>,
    pub(crate) poster: Option<ImageRef>,
}

/// A Show looked up in the title source, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct FoundShow {
    /// The id its Season Requests are kept under.
    pub(crate) id: ExternalId,
    /// The Plugin whose title source gave the Show, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) show: SourceShow,
    /// Its Seasons, in order, as the viewer sees them.
    pub(crate) seasons: Vec<FoundSeason>,
    /// Its open Followed Show, if it has one the viewer may see.
    pub(crate) followed_show: Option<FollowedShow>,
    /// Its Quality, which each of its Season Requests takes, if it has one.
    pub(crate) quality: Option<RequestedQuality>,
}

impl Requests {
    /// The Movies the title source finds titled like `title`, best fit first, as `viewer`
    /// sees them.
    pub(crate) async fn search(
        &self,
        viewer: &User,
        title: String,
    ) -> Result<Vec<Found<Candidate>>, Error> {
        let candidates = self.find(TitleKind::Movie, title).await?;
        let mut conn = self.db.reader().acquire().await?;
        let mut found = Vec::with_capacity(candidates.len());
        // ponytail: two queries per Movie found, which a search caps at a page of 20.
        for Hit {
            id,
            plugin,
            candidate,
        } in candidates
        {
            let movie = self
                .look_around(&mut conn, viewer, id, plugin, candidate)
                .await?;
            found.push(movie);
        }
        Ok(found)
    }

    /// The Movies the title source finds for a search for `query`, as `viewer` sees them, less any
    /// with an id in `in_libraries`, ranked as ADR 0080 says: see [`ranked`]. `None`, without
    /// asking, when `viewer` may not request Movies, so has none to find.
    pub(crate) async fn search_to_request(
        &self,
        viewer: &User,
        query: &str,
        in_libraries: &HashSet<ExternalId>,
    ) -> Result<Option<Vec<Found<Candidate>>>, Error> {
        if !viewer.permissions.allow(Permission::RequestMovie) {
            return Ok(None);
        }
        let found = self.search(viewer, query.to_owned()).await?;
        Ok(Some(ranked(found, in_libraries, query, movie_standing)))
    }

    /// The Shows the title source finds for a search for `query`, as `viewer` sees them, less any
    /// with an id in `in_libraries`, ranked as [`Self::search_to_request`] ranks Movies. `None`,
    /// without asking, when `viewer` may not request Shows.
    pub(crate) async fn search_shows_to_request(
        &self,
        viewer: &User,
        query: &str,
        in_libraries: &HashSet<ExternalId>,
    ) -> Result<Option<Vec<ShowToRequest>>, Error> {
        if !viewer.permissions.allow(Permission::RequestTv) {
            return Ok(None);
        }
        let hits = self.search_shows(query.to_owned()).await?;
        let mut conn = self.db.reader().acquire().await?;
        let mut found = Vec::with_capacity(hits.len());
        // ponytail: four or five queries per Show found, which a search caps at a page of 20.
        for hit in hits {
            found.push(self.look_around_show(&mut conn, viewer, hit).await?);
        }
        Ok(Some(ranked(found, in_libraries, query, show_standing)))
    }

    /// The Shows the title source finds titled like `title`, best fit first.
    pub(crate) async fn search_shows(&self, title: String) -> Result<Vec<Hit>, Error> {
        self.find(TitleKind::Show, title).await
    }

    /// The titles of `kind` the first enabled title source to find any finds titled like `title`,
    /// best fit first: a search belongs to no Library, so it asks in Admin precedence.
    async fn find(&self, kind: TitleKind, title: String) -> Result<Vec<Hit>, Error> {
        let query = SearchQuery {
            kind,
            title,
            year: None,
        };
        let (source, candidates) = match self.metadata.search_in_turn(&query).await {
            Ok(found) => found,
            Err(Unanswered::NotFound) => return Ok(Vec::new()),
            Err(Unanswered::Unavailable) => return Err(Error::Unavailable),
        };
        Ok(candidates
            .into_iter()
            .map(|candidate| Hit {
                id: candidate.id.clone(),
                plugin: source.plugin.clone(),
                candidate,
            })
            .collect())
    }

    /// The Show `asked` and each of its Seasons, fetched now, as `viewer` sees them.
    pub(crate) async fn look_up_show(
        &self,
        viewer: &User,
        asked: &ExternalId,
    ) -> Result<FoundShow, Error> {
        let (source, show) = self.show(asked).await?;
        let id = kept_under(asked, &show.ids);
        let seasons = self.seasons_from(&source, asked, &show).await?;
        let mut conn = self.db.reader().acquire().await?;
        let seasons = self
            .look_around_seasons(&mut conn, viewer, &id, seasons)
            .await?;
        let followed_show = match open_followed_show(&mut conn, &id).await? {
            Some(followed) => load_followed(&mut conn, Some(followed), viewer.whose_requests())
                .await?
                .pop(),
            None => None,
        };
        let mut ids = show.ids.clone();
        ids.insert(id.clone());
        let quality = show_requests(&mut conn, &ids).await?.quality().cloned();
        Ok(FoundShow {
            id,
            plugin: source.plugin,
            show,
            seasons,
            followed_show,
            quality,
        })
    }

    /// The Movie `asked`, fetched now as [`Self::look_up`] fetches it, to list.
    pub(crate) async fn listed_movie(&self, asked: &ExternalId) -> Result<Listed, Error> {
        let movie = self.fetch(asked).await?;
        Ok(Listed {
            id: kept_under(asked, &movie.ids),
            year: movie.release_date.as_deref().and_then(year_of),
            title: movie.title,
            poster: movie.poster,
        })
    }

    /// The Show `asked`, fetched now as [`Self::look_up_show`] fetches it, less its Seasons, to
    /// list.
    pub(crate) async fn listed_show(&self, asked: &ExternalId) -> Result<Listed, Error> {
        let (source, show) = self.show(asked).await?;
        Ok(Listed {
            id: kept_under(asked, &show.ids),
            year: show.first_air_date.as_deref().and_then(year_of),
            poster: ImageRef::of(&source.plugin, show.poster.as_ref()),
            title: show.title,
        })
    }

    /// The open Request for the Movie kept under `id`, if `viewer` may see it.
    pub(crate) async fn open_movie_request(
        &self,
        viewer: &User,
        id: &ExternalId,
    ) -> Result<Option<Request>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        self.visible_open_request(&mut conn, viewer, id, None).await
    }

    /// The Show `asked`, from the title source keeping titles under its namespace, else the first
    /// one looking them up by it to have it, with that source.
    pub(super) async fn show(&self, asked: &ExternalId) -> Result<(Source, SourceShow), Error> {
        if self.metadata.unserved(asked, TitleMethod::Show).await {
            return Err(Error::NoSuchShow(asked.clone()));
        }
        self.metadata
            .show_by(asked)
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(asked.clone())))
    }

    /// The Season `number` of `show`, which `source` gave when asked by `asked`, from that source
    /// by the Show's id in its namespace, else by `asked`: the id a Request is kept under may be
    /// one `source` doesn't know.
    pub(super) async fn season_from(
        &self,
        source: &Source,
        asked: &ExternalId,
        show: &SourceShow,
        number: u32,
    ) -> Result<SourceSeason, Unanswered> {
        let id = show
            .ids
            .get(&source.namespace)
            .unwrap_or_else(|| asked.clone());
        self.metadata.season_from(source, &id, number).await
    }

    /// Each of the Seasons `show` lists, fetched now as [`Self::season_from`] fetches one. One it
    /// lists but can't give fails the lookup as an unknown Show.
    pub(super) async fn seasons_from(
        &self,
        source: &Source,
        asked: &ExternalId,
        show: &SourceShow,
    ) -> Result<Vec<SourceSeason>, Error> {
        let mut seasons = Vec::with_capacity(show.seasons.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in &show.seasons {
            let season = self.season_from(source, asked, show, number).await;
            seasons.push(season.map_err(|err| looking_up(err, Error::NoSuchShow(asked.clone())))?);
        }
        Ok(seasons)
    }

    /// The Movie `asked`, fetched now, as `viewer` sees it.
    pub(crate) async fn look_up(
        &self,
        viewer: &User,
        asked: &ExternalId,
    ) -> Result<Found<Metadata>, Error> {
        let movie = self.fetch(asked).await?;
        let mut conn = self.db.reader().acquire().await?;
        let plugin = movie.plugin.clone();
        let id = kept_under(asked, &movie.ids);
        self.look_around(&mut conn, viewer, id, plugin, movie)
            .await
            .map_err(Error::from)
    }

    /// The Movie `id` from the title source keeping titles under its namespace, else the first one
    /// looking them up by it to have it, failing with [`Error::NoSuchMovie`] or
    /// [`Error::Unavailable`] as [`looking_up`] maps it.
    pub(super) async fn fetch(&self, id: &ExternalId) -> Result<Metadata, Error> {
        if self.metadata.unserved(id, TitleMethod::Movie).await {
            return Err(Error::NoSuchMovie(id.clone()));
        }
        self.metadata
            .movie_by(id)
            .await
            .map(|(_, movie)| movie)
            .map_err(|err| looking_up(err, Error::NoSuchMovie(id.clone())))
    }

    /// `movie`, the Movie `id` as `plugin`'s title source gave it, with the Library that has it and
    /// its open Request, if `viewer` may see it.
    #[expect(
        clippy::too_many_arguments,
        reason = "`Found` takes the Movie as a search or lookup gives it, in three parts"
    )]
    async fn look_around<M>(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        id: ExternalId,
        plugin: PluginId,
        movie: M,
    ) -> Result<Found<M>, sqlx::Error> {
        let library = library_with_movie(conn, &id).await?;
        let request = self.visible_open_request(conn, viewer, &id, None).await?;
        Ok(Found {
            id,
            plugin,
            movie,
            library,
            request,
        })
    }

    /// The Show `hit` with the Library that has an Episode of it, and its open Season Request and
    /// Followed Show, if `viewer` may see them.
    async fn look_around_show(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        hit: Hit,
    ) -> Result<ShowToRequest, sqlx::Error> {
        let mut ids = hit.candidate.ids.clone();
        ids.insert(hit.id.clone());
        let library = library_with_show(conn, &hit.id).await?;
        let request = self.visible_open_season_request(conn, viewer, &ids).await?;
        let followed = match open_followed_show_by_any(conn, &ids).await? {
            Some(id) => !(load_followed(conn, Some(id), viewer.whose_requests()).await?).is_empty(),
            None => false,
        };
        Ok(ShowToRequest {
            hit,
            library,
            request,
            followed,
        })
    }

    /// Each of `seasons`, of the Show `show`, as `viewer` sees it: whether a Library has it, and its
    /// open Request.
    async fn look_around_seasons(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        show: &ExternalId,
        seasons: Vec<SourceSeason>,
    ) -> Result<Vec<FoundSeason>, sqlx::Error> {
        let today = today(conn).await?;
        let mut found = Vec::with_capacity(seasons.len());
        // ponytail: two or three queries per Season, and a Show has a few dozen at most.
        for season in seasons {
            let matched = matched_episodes(conn, show, season.number.into()).await?;
            let request = self
                .visible_open_request(conn, viewer, show, Some(season.number))
                .await?;
            found.push(FoundSeason {
                season_number: season.number,
                in_library: availability(&season.episodes, &matched, &today)
                    == Some(RequestState::Available),
                air_date: season.air_date,
                request: request.into(),
                ..Default::default()
            });
        }
        Ok(found)
    }
}

/// The year of a `YYYY-MM-DD` date.
pub(crate) fn year_of(date: &str) -> Option<i32> {
    date.get(..4)?.parse().ok()
}

/// `err` from looking a title up, which is `missing` if the title source doesn't have it.
pub(super) fn looking_up(err: Unanswered, missing: Error) -> Error {
    match err {
        Unanswered::NotFound => missing,
        Unanswered::Unavailable => Error::Unavailable,
    }
}

/// A title the title source found, whether a Library has it, and whether the viewer sees it
/// Requested, which [`ranked`] orders by.
type Standing<'a> = (&'a Candidate, bool, bool);

fn movie_standing(found: &Found<Candidate>) -> Standing<'_> {
    (
        &found.movie,
        found.library.is_some(),
        found.request.is_some(),
    )
}

fn show_standing(found: &ShowToRequest) -> Standing<'_> {
    let requested = found.request.is_some() || found.followed;
    (&found.hit.candidate, found.library.is_some(), requested)
}

/// `found`, in the title source's order, less any with an id in `in_libraries`: those a Library
/// has first, then those Requested, as `standing` tells, then the rest, each by how well the
/// title fits `query`, as the Libraries' search bands it, then in the source's order.
fn ranked<T>(
    found: Vec<T>,
    in_libraries: &HashSet<ExternalId>,
    query: &str,
    standing: impl Fn(&T) -> Standing<'_>,
) -> Vec<T> {
    let mut found: Vec<_> = (found.into_iter())
        .filter(|found| {
            let (title, ..) = standing(found);
            !in_libraries.contains(&title.id)
                && !title.ids.iter().any(|id| in_libraries.contains(&id))
        })
        .collect();
    found.sort_by_cached_key(|found| {
        let (title, available, requested) = standing(found);
        let year = title.year.map(|year| year.to_string());
        let bands = match_bands(query, &title.title, year.as_deref());
        (!available, !requested, bands)
    });
    found
}

#[cfg(test)]
#[path = "lookup_tests.rs"]
mod tests;
