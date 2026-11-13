//! Looking titles up in the metadata source for Requests, as a viewer sees them.

use nama_plugin_api::{
    PluginId,
    metadata::{Candidate, ExternalId, SearchQuery, Season, Show, TitleKind},
};
use nama_proto::nama::v1::{FollowedShow, FoundSeason, Request, RequestState, RequestedQuality};
use sqlx::SqliteConnection;

use super::availability::{availability, today};
use super::listing::visible_to;
use super::quality::show_requests;
use super::{Error, Requests, load_followed, open_followed_show};
use crate::{
    library::contents::{self, Metadata},
    metadata,
    users::User,
};

/// A Movie the metadata source has, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct Found<M> {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider gave the Movie, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) movie: M,
    /// The oldest Library with the Movie Matched, if any.
    pub(crate) library: Option<i64>,
    /// The Movie's open Request, if it has one the viewer may see.
    pub(crate) request: Option<Request>,
}

/// A title a search found, which a Request can name by its TMDB id.
#[derive(Debug)]
pub(crate) struct Hit {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider found it, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) candidate: Candidate,
}

/// A Show looked up in the metadata source, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct FoundShow {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider gave the Show, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) show: Show,
    /// Its Seasons, in order, as the viewer sees them.
    pub(crate) seasons: Vec<FoundSeason>,
    /// Its open Followed Show, if it has one the viewer may see.
    pub(crate) followed_show: Option<FollowedShow>,
    /// Its Quality, which each of its Season Requests takes, if it has one.
    pub(crate) quality: Option<RequestedQuality>,
}

impl Requests {
    /// The Movies the metadata source finds titled like `title`, best fit first, as `viewer`
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
            tmdb_id,
            plugin,
            candidate,
        } in candidates
        {
            let movie = self
                .look_around(&mut conn, viewer, tmdb_id, plugin, candidate)
                .await?;
            found.push(movie);
        }
        Ok(found)
    }

    /// The Shows the metadata source finds titled like `title`, best fit first.
    pub(crate) async fn search_shows(&self, title: String) -> Result<Vec<Hit>, Error> {
        self.find(TitleKind::Show, title).await
    }

    /// The titles of `kind` the metadata source finds titled like `title`, best fit first.
    async fn find(&self, kind: TitleKind, title: String) -> Result<Vec<Hit>, Error> {
        let query = SearchQuery {
            kind,
            title,
            year: None,
        };
        let (plugin, candidates) = match self.metadata.search(&query).await {
            Ok(found) => found,
            Err(metadata::Error::NotFound) => return Ok(Vec::new()),
            Err(metadata::Error::Unavailable) => return Err(Error::Unavailable),
        };
        // Requests name titles by TMDB id, so a title without one can't be asked for.
        Ok(candidates
            .into_iter()
            .filter_map(|candidate| match candidate.id {
                ExternalId::Tmdb(tmdb_id) => Some(Hit {
                    tmdb_id,
                    plugin: plugin.clone(),
                    candidate,
                }),
                ExternalId::Imdb(_) | ExternalId::Tvdb(_) => None,
            })
            .collect())
    }

    /// The Show with the TMDB id `tmdb_id` and each of its Seasons, fetched now, as `viewer` sees
    /// them.
    pub(crate) async fn look_up_show(
        &self,
        viewer: &User,
        tmdb_id: i64,
    ) -> Result<FoundShow, Error> {
        let (plugin, show) = self
            .metadata
            .show(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let seasons = self.seasons(tmdb_id, &show).await?;
        let mut conn = self.db.reader().acquire().await?;
        let seasons = self
            .look_around_seasons(&mut conn, viewer, tmdb_id, seasons)
            .await?;
        let followed_show = match open_followed_show(&mut conn, tmdb_id).await? {
            Some(id) => load_followed(&mut conn, Some(id), visible_to(viewer))
                .await?
                .pop(),
            None => None,
        };
        let quality = show_requests(&mut conn, tmdb_id).await?.quality().cloned();
        Ok(FoundShow {
            tmdb_id,
            plugin,
            show,
            seasons,
            followed_show,
            quality,
        })
    }

    /// Each of the Seasons `show`, the Show `tmdb_id`, lists, fetched now. One it lists but can't
    /// give fails the lookup as an unknown Show.
    pub(super) async fn seasons(&self, tmdb_id: i64, show: &Show) -> Result<Vec<Season>, Error> {
        let id = ExternalId::Tmdb(tmdb_id);
        let mut seasons = Vec::with_capacity(show.seasons.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in &show.seasons {
            let season = self.metadata.season(&id, number).await;
            seasons.push(season.map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?);
        }
        Ok(seasons)
    }

    /// The Movie with the TMDB id `tmdb_id`, fetched now, as `viewer` sees it.
    pub(crate) async fn look_up(
        &self,
        viewer: &User,
        tmdb_id: i64,
    ) -> Result<Found<Metadata>, Error> {
        let movie = self.fetch(tmdb_id).await?;
        let mut conn = self.db.reader().acquire().await?;
        let plugin = movie.plugin.clone();
        self.look_around(&mut conn, viewer, tmdb_id, plugin, movie)
            .await
            .map_err(Error::from)
    }

    pub(super) async fn fetch(&self, tmdb_id: i64) -> Result<Metadata, Error> {
        self.metadata
            .movie(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchMovie(tmdb_id)))
    }

    /// `movie`, the Movie `tmdb_id` as `plugin`'s Provider gave it, with the Library that has it and
    /// its open Request, if `viewer` may see it.
    #[expect(
        clippy::too_many_arguments,
        reason = "`Found` takes the Movie as a search or lookup gives it, in three parts"
    )]
    async fn look_around<M>(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        tmdb_id: i64,
        plugin: PluginId,
        movie: M,
    ) -> Result<Found<M>, sqlx::Error> {
        let library = contents::library_with_movie(conn, tmdb_id).await?;
        let request = self
            .visible_open_request(conn, viewer, tmdb_id, None)
            .await?;
        Ok(Found {
            tmdb_id,
            plugin,
            movie,
            library,
            request,
        })
    }

    /// Each of `seasons`, of the Show `show_tmdb_id`, as `viewer` sees it: whether a Library has it,
    /// and its open Request.
    async fn look_around_seasons(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        show_tmdb_id: i64,
        seasons: Vec<Season>,
    ) -> Result<Vec<FoundSeason>, sqlx::Error> {
        let today = today(conn).await?;
        let mut found = Vec::with_capacity(seasons.len());
        // ponytail: two or three queries per Season, and a Show has a few dozen at most.
        for season in seasons {
            let matched =
                contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
            let request = self
                .visible_open_request(conn, viewer, show_tmdb_id, Some(season.number))
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

/// `err` from looking a title up, which is `missing` if the metadata source doesn't have it.
pub(super) fn looking_up(err: metadata::Error, missing: Error) -> Error {
    match err {
        metadata::Error::NotFound => missing,
        metadata::Error::Unavailable => Error::Unavailable,
    }
}
