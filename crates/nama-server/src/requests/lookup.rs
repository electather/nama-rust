//! Looking titles up in the metadata source for Requests, as a viewer sees them.

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, TitleMethod},
};
use nama_proto::nama::v1::{FollowedShow, FoundSeason, Request, RequestState, RequestedQuality};
use sqlx::SqliteConnection;

use super::asking::kept_under;
use super::availability::{availability, today};
use super::followed::{load_followed, open_followed_show};
use super::quality::show_requests;
use super::{Error, Requests};
use crate::{
    library::{
        Candidate, Metadata, SearchQuery, Source, SourceSeason, SourceShow, TitleKind, Unanswered,
        library_with_movie, matched_episodes,
    },
    users::User,
};

/// A Movie the metadata source has, and how close it is to being on the Server.
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

/// A title a search found, which a Request can name by its id.
#[derive(Debug)]
pub(crate) struct Hit {
    pub(crate) id: ExternalId,
    /// The Plugin whose title source found it, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) candidate: Candidate,
}

/// A Show looked up in the metadata source, and how close it is to being on the Server.
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

    /// The Shows the metadata source finds titled like `title`, best fit first.
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

/// `err` from looking a title up, which is `missing` if the metadata source doesn't have it.
pub(super) fn looking_up(err: Unanswered, missing: Error) -> Error {
    match err {
        Unanswered::NotFound => missing,
        Unanswered::Unavailable => Error::Unavailable,
    }
}
