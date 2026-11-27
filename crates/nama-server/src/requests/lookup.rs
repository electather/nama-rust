//! Looking titles up in the title source for Requests, as a viewer sees them.

use std::collections::HashSet;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds, TitleMethod},
};
use nama_proto::nama::v1::{
    FollowedShow, FoundSeason, Permission, Request, RequestState, RequestedQuality,
};
use sqlx::SqliteConnection;

use super::asked::{Asked, title_seen};
use super::asking::kept_under;
use super::availability::{availability, today};
use super::followed::{load_followed, open_followed_show, open_followed_show_by_any};
use super::quality::show_requests;
use super::{Error, Requests};
use crate::{
    library::{
        Candidate, ImageRef, Metadata, SearchQuery, Source, SourceEpisode, SourceSeason,
        SourceShow, TitleKind, Unanswered, Whereabouts, matched_episodes,
    },
    users::{Reach, User, reach},
};

/// A Movie the title source has, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct Found<M> {
    /// The id its Request is kept under.
    pub(crate) id: ExternalId,
    /// The Plugin whose title source gave the Movie, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) movie: M,
    /// The Library the viewer sees it in, as [`Whereabouts`] says, if any.
    pub(crate) library: Option<i64>,
    /// The Movie's open Request, if it has one the viewer may see.
    pub(crate) request: Option<Request>,
}

/// A Show the title source has, and how close it is to being on the Server.
#[derive(Debug)]
pub(crate) struct ShowToRequest {
    pub(crate) hit: Hit,
    /// The Library the viewer sees it in, as [`Whereabouts`] says, if any.
    pub(crate) library: Option<i64>,
    /// The open Request for its earliest Season with one the viewer may see, if any.
    pub(crate) request: Option<Request>,
    /// Whether it has an open Followed Show the viewer may see.
    pub(crate) followed: bool,
}

impl ShowToRequest {
    /// Whether the viewer sees it Requested: through a Season Request or its Followed Show.
    pub(crate) fn requested(&self) -> bool {
        self.request.is_some() || self.followed
    }
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
    pub(crate) content_rating: Option<String>,
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
    pub(crate) seasons: Vec<LookedUpSeason>,
    /// Its open Followed Show, if it has one the viewer may see.
    pub(crate) followed_show: Option<FollowedShow>,
    /// Its Quality, which each of its Season Requests takes, if it has one.
    pub(crate) quality: Option<RequestedQuality>,
}

/// A Season of a [`FoundShow`], as the viewer sees it, with its Episodes.
#[derive(Debug)]
pub(crate) struct LookedUpSeason {
    /// The Season as clients see it, less its Episodes, whose Stills need offering to them.
    pub(crate) season: FoundSeason,
    /// Its Episodes as the title source gave them, in order, each with whether a Library has a
    /// file Matched to it.
    pub(crate) episodes: Vec<(SourceEpisode, bool)>,
}

impl Requests {
    /// The Movies the title source finds titled like `title`, best fit first, as `viewer`
    /// sees them: leaving out those [`shown_hit`] doesn't show them.
    pub(crate) async fn search(
        &self,
        viewer: &User,
        title: String,
    ) -> Result<Vec<Found<Candidate>>, Error> {
        let candidates = self.find(TitleKind::Movie, title).await?;
        let reach = reach(&self.db, viewer.id).await?;
        let mut conn = self.db.reader().acquire().await?;
        let mut found = Vec::with_capacity(candidates.len());
        // ponytail: two queries per Movie found, which a search caps at a page of 20.
        for Hit {
            id,
            plugin,
            candidate,
        } in candidates
        {
            let placed = Whereabouts::of_title(&mut conn, &reach, TitleKind::Movie, &id).await?;
            if !shown_hit(&reach, &placed) {
                continue;
            }
            let library = placed.reached();
            let movie =
                (self.look_around(&mut conn, viewer, id, plugin, candidate, library)).await?;
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
        Ok(Some(ranked(found, in_libraries, movie_standing)))
    }

    /// The Shows the title source finds for a search for `query`, as `viewer` sees them, less any
    /// with an id in `in_libraries`, ranked as [`Self::search_to_request`] ranks Movies, leaving
    /// out those [`shown_hit`] doesn't show them. `None`, without asking, when `viewer` may not
    /// request Shows.
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
        let reach = reach(&self.db, viewer.id).await?;
        let mut conn = self.db.reader().acquire().await?;
        let mut found = Vec::with_capacity(hits.len());
        // ponytail: a few queries per Show found, more with open Season Requests the viewer can't
        // see, and a search caps them at a page of 20.
        for hit in hits {
            let placed = Whereabouts::of_title(&mut conn, &reach, TitleKind::Show, &hit.id).await?;
            if shown_hit(&reach, &placed) {
                let library = placed.reached();
                found.push(
                    self.look_around_show(&mut conn, viewer, hit, library)
                        .await?,
                );
            }
        }
        Ok(Some(ranked(found, in_libraries, show_standing)))
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

    /// The Show `asked` and each of its Seasons, fetched now, as `viewer` sees them:
    /// [`Error::NoSuchShow`] if it is kept from them ([`judged`]).
    pub(crate) async fn look_up_show(
        &self,
        viewer: &User,
        asked: &ExternalId,
    ) -> Result<FoundShow, Error> {
        let (source, show) = self.show(asked).await?;
        let id = kept_under(asked, &show.ids);
        let reach = reach(&self.db, viewer.id).await?;
        let placed = self.placed(&reach, TitleKind::Show, &id).await?;
        let rating = show.content_rating.as_deref();
        if matches!(judged(placed, &reach, rating), Whereabouts::KeptFrom) {
            return Err(Error::NoSuchShow(asked.clone()));
        }
        // ponytail: every Season from the title source on each call, and the web app calls again
        // on each `RequestProgress`, about once a minute while a Season downloads. Cache Seasons
        // here if the title source's rate limit bites.
        let seasons = self.seasons_from(&source, asked, &show).await?;
        // Taken after asking the title source, so no pooled reader waits on a Plugin.
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
            content_rating: movie.content_rating,
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
            content_rating: show.content_rating,
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

    /// The Movie `asked`, fetched now, as `viewer` sees it, in the Library they see it in:
    /// [`Error::NoSuchMovie`] if it is kept from them ([`judged`]).
    pub(crate) async fn look_up(
        &self,
        viewer: &User,
        asked: &ExternalId,
    ) -> Result<Found<Metadata>, Error> {
        let movie = self.fetch(asked).await?;
        let id = kept_under(asked, &movie.ids);
        let reach = reach(&self.db, viewer.id).await?;
        let placed = self.placed(&reach, TitleKind::Movie, &id).await?;
        let library = match judged(placed, &reach, movie.content_rating.as_deref()) {
            Whereabouts::KeptFrom => return Err(Error::NoSuchMovie(asked.clone())),
            Whereabouts::In(library) => Some(library),
            Whereabouts::Nowhere => None,
        };
        let mut conn = self.db.reader().acquire().await?;
        let plugin = movie.plugin.clone();
        Ok(self
            .look_around(&mut conn, viewer, id, plugin, movie, library)
            .await?)
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

    /// `movie`, the Movie `id` as `plugin`'s title source gave it, with `library`, the one the
    /// viewer sees it in, and its open Request, if `viewer` may see it.
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
        library: Option<i64>,
    ) -> Result<Found<M>, sqlx::Error> {
        let request = self.visible_open_request(conn, viewer, &id, None).await?;
        Ok(Found {
            id,
            plugin,
            movie,
            library,
            request,
        })
    }

    /// The Show `hit` with `library`, the one the viewer sees it in, and its open Season Request
    /// and Followed Show, if `viewer` may see them.
    async fn look_around_show(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        hit: Hit,
        library: Option<i64>,
    ) -> Result<ShowToRequest, sqlx::Error> {
        let mut ids = hit.candidate.ids.clone();
        ids.insert(hit.id.clone());
        let request = self.visible_open_season_request(conn, viewer, &ids).await?;
        let followed = self.sees_followed_show(conn, viewer, &ids).await?;
        Ok(ShowToRequest {
            hit,
            library,
            request,
            followed,
        })
    }

    /// Whether the Show with any of `ids` has an open Followed Show `viewer` may see, as
    /// [`title_seen`] judges it.
    pub(super) async fn sees_followed_show(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        ids: &ExternalIds,
    ) -> Result<bool, sqlx::Error> {
        let Some(id) = open_followed_show_by_any(conn, ids).await? else {
            return Ok(false);
        };
        let Some(followed) = load_followed(conn, Some(id), viewer.whose_requests())
            .await?
            .pop()
        else {
            return Ok(false);
        };
        let reach = reach(&self.db, viewer.id).await?;
        let requesters = &followed.requesters;
        title_seen(conn, viewer, &reach, Asked::FollowedShow, id, requesters).await
    }

    /// Each of `seasons`, of the Show `show`, as `viewer` sees it: whether a Library has it, and its
    /// open Request.
    async fn look_around_seasons(
        &self,
        conn: &mut SqliteConnection,
        viewer: &User,
        show: &ExternalId,
        seasons: Vec<SourceSeason>,
    ) -> Result<Vec<LookedUpSeason>, sqlx::Error> {
        let today = today(conn).await?;
        let mut found = Vec::with_capacity(seasons.len());
        // ponytail: two or three queries per Season, and a Show has a few dozen at most.
        for season in seasons {
            let matched = matched_episodes(conn, show, season.number.into()).await?;
            let request = self
                .visible_open_request(conn, viewer, show, Some(season.number))
                .await?;
            let in_library = availability(&season.episodes, &matched, &today);
            let found_season = FoundSeason {
                season_number: season.number,
                in_library: in_library == Some(RequestState::Available),
                air_date: season.air_date,
                request: request.into(),
                ..Default::default()
            };
            let episodes = (season.episodes.into_iter())
                .map(|episode| {
                    let has = matched.contains(&i64::from(episode.number));
                    (episode, has)
                })
                .collect();
            found.push(LookedUpSeason {
                season: found_season,
                episodes,
            });
        }
        Ok(found)
    }

    /// Where the title of `kind` known by `id` is for a viewer whose Reach is `reach`.
    pub(super) async fn placed(
        &self,
        reach: &Reach,
        kind: TitleKind,
        id: &ExternalId,
    ) -> Result<Whereabouts<i64>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        Whereabouts::of_title(&mut conn, reach, kind, id).await
    }

    /// Whether `viewer`'s Maximum Age keeps from them a title the title source rates `rating`:
    /// to them there is no such title, neither to look up nor to Request.
    pub(super) async fn kept_from(
        &self,
        viewer: &User,
        rating: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        Ok(!reach(&self.db, viewer.id).await?.allows_rating(rating))
    }
}

/// `placed`, for a viewer whose Reach is `reach`, with a title in no Library judged by `rating`,
/// the title source's: [`Whereabouts::KeptFrom`] means that to them there is no such title,
/// neither to look up nor to Request.
pub(super) fn judged(
    placed: Whereabouts<i64>,
    reach: &Reach,
    rating: Option<&str>,
) -> Whereabouts<i64> {
    match placed {
        Whereabouts::Nowhere if !reach.allows_rating(rating) => Whereabouts::KeptFrom,
        Whereabouts::In(_) | Whereabouts::KeptFrom | Whereabouts::Nowhere => placed,
    }
}

/// Whether a title source's hit, `placed` for a viewer whose Reach is `reach`, shows in their
/// search. A hit has no Content Rating, so it counts as unrated, and one in a Library is left to
/// Browse's search, which knows its stored one, unless no Maximum Age judges it.
fn shown_hit(reach: &Reach, placed: &Whereabouts<i64>) -> bool {
    match placed {
        Whereabouts::In(_) => reach.lets_every_rating(),
        Whereabouts::KeptFrom => false,
        Whereabouts::Nowhere => reach.allows_unrated(),
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
    (
        &found.hit.candidate,
        found.library.is_some(),
        found.requested(),
    )
}

/// `found`, less any with an id in `in_libraries`: those a Library has first, then those
/// Requested, as `standing` tells, then the rest, each in the title source's order, which puts
/// its best known titles first.
fn ranked<T>(
    found: Vec<T>,
    in_libraries: &HashSet<ExternalId>,
    standing: impl Fn(&T) -> Standing<'_>,
) -> Vec<T> {
    let mut found: Vec<_> = (found.into_iter())
        .filter(|found| {
            let (title, ..) = standing(found);
            !in_libraries.contains(&title.id)
                && !title.ids.iter().any(|id| in_libraries.contains(&id))
        })
        .collect();
    found.sort_by_key(|found| {
        let (_, available, requested) = standing(found);
        (!available, !requested)
    });
    found
}

#[cfg(test)]
#[path = "lookup_tests.rs"]
mod tests;
