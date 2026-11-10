//! Requests: Users asking for a Movie, or a Season of a Show, that isn't on the Server to be added
//! to it (ADR 0012).
//!
//! Followed Shows sit beside them: Users asking for a whole Show, whose later Seasons get Season
//! Requests as a Library scan Matches their Episodes, never by asking a metadata source.
//!
//! This module is the only code that reads or writes `requests`, `requesters`, `followed_shows`
//! and `followed_show_requesters`. It hands Requests out as their proto messages, since those are
//! also what its events carry.

use std::{
    cmp::Reverse,
    collections::{BTreeSet, HashMap, HashSet},
    iter,
    sync::Arc,
    time::Duration,
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_plugin_api::{
    PluginId,
    fulfiller::{Fulfilled, Movies, Shows},
    metadata::{Candidate, Episode, ExternalId, ExternalIds, SearchQuery, Season, Show, TitleKind},
};
use nama_proto::nama::v1::{
    DefaultQuality, Event, ExtensionPoint, FollowedShow, FollowedShowCancelled,
    FollowedShowCreated, FollowedShowRequesterAdded, FollowedShowRequesterRemoved,
    FollowedShowStateChanged, FollowedShowWithdrawn, FoundSeason, ListRequestsResponse, Permission,
    Quality, QualityChange, Request, RequestCancelled, RequestCreated, RequestState,
    RequestStateChanged, RequestWithdrawn, RequestedMovie, RequestedQuality, RequestedSeason,
    Requester, RequesterAdded, RequesterRemoved, event::Event as Happened,
};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    contents::{self, Contents, Metadata},
    db::{BoxFuture, Database, detached, now},
    events::{EventBus, event},
    matching::{ManualError, Matcher},
    plugins::Registry,
    qualities,
    users::User,
};

mod hand_off;

use hand_off::HandOff;
pub(crate) use hand_off::PROGRESS_EVERY;

/// Each Request state as `requests.state` stores it.
const STATES: [(RequestState, &str); 7] = [
    (RequestState::Pending, "pending"),
    (RequestState::Approved, "approved"),
    (RequestState::Declined, "declined"),
    (RequestState::Processing, "processing"),
    (RequestState::PartiallyAvailable, "partially_available"),
    (RequestState::Available, "available"),
    (RequestState::Failed, "failed"),
];

/// Why a Request couldn't be made, found or changed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// There is no such Request, or none the caller may see.
    #[error("there is no Request {0}")]
    NoSuchRequest(i64),
    #[error("the Request {0} isn't Pending")]
    NotPending(i64),
    #[error("the Request {0} isn't Failed")]
    NotFailed(i64),
    #[error("you aren't a Requester of the Request {0}")]
    NotARequester(i64),
    #[error("a Library already has the Movie tmdb-{0}")]
    InLibrary(i64),
    #[error("every aired Episode of Season {season} of the Show tmdb-{show} is in a Library")]
    SeasonInLibrary { show: i64, season: u32 },
    #[error("the metadata source has no Movie tmdb-{0}")]
    NoSuchMovie(i64),
    #[error("the metadata source has no Show tmdb-{0}")]
    NoSuchShow(i64),
    #[error("the metadata source has no Season {season} of the Show tmdb-{show}")]
    NoSuchSeason { show: i64, season: u32 },
    /// No metadata Provider is enabled, or it failed.
    #[error("no metadata Provider could answer")]
    Unavailable,
    /// There is no such Followed Show, or none the caller may see.
    #[error("there is no Followed Show {0}")]
    NoSuchFollowedShow(i64),
    #[error("the Followed Show {0} isn't Pending")]
    FollowedShowNotPending(i64),
    #[error("you aren't a Requester of the Followed Show {0}")]
    NotAFollowedShowRequester(i64),
    #[error(transparent)]
    Quality(#[from] qualities::Refused),
    #[error("the page token isn't one this Server gave for listing in this state")]
    PageTokenInvalid,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The Server's Requests, which publishes what changes about them.
#[derive(Debug, Clone)]
pub(crate) struct Requests {
    db: Arc<Database>,
    contents: Contents,
    /// Looks up what a new Request asks for: a Movie, or a Show's Seasons.
    matcher: Matcher,
    events: EventBus,
    /// Its only way to the Fulfillers.
    hand_off: HandOff,
}

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

/// A Show the metadata source has, as `S` holds it.
#[derive(Debug)]
pub(crate) struct FoundShow<S> {
    pub(crate) tmdb_id: i64,
    /// The Plugin whose metadata Provider gave the Show, which its image paths are for.
    pub(crate) plugin: PluginId,
    pub(crate) show: S,
    /// Its Seasons, in order, as the viewer sees them. Only a lookup fills them.
    pub(crate) seasons: Vec<FoundSeason>,
    /// Its open Followed Show, if it has one the viewer may see. Only a lookup fills it.
    pub(crate) followed_show: Option<FollowedShow>,
    /// Its Quality, which each of its Season Requests takes, if it has one. Only a lookup fills
    /// it.
    pub(crate) quality: Option<RequestedQuality>,
}

/// What a new Request asks for, as the metadata source gave it.
#[derive(Debug)]
struct Wanted {
    /// The Movie's TMDB id, or for a Season its Show's.
    tmdb_id: i64,
    /// The Season's number, or `None` for a Movie.
    season: Option<u32>,
    /// The Movie's title, or for a Season its Show's.
    title: String,
    /// The Movie's `YYYY-MM-DD` release date, or the Season's first air date.
    release_date: Option<String>,
    /// For a Season, its Show's ids; none for a Movie.
    show_ids: ExternalIds,
    /// The Followed Show a new Request for it is opened for, if any.
    followed_show: Option<i64>,
    /// The Quality a new Request for it takes, if its Fulfiller has any.
    quality: Option<RequestedQuality>,
    /// Whether the asker chose `quality`, by picking it or as their personal default, so it
    /// replaces a Pending Request's when their ask approves it.
    quality_chosen: bool,
}

/// A User changing a Request's Quality: a Request manager as they approve or retry it, or an
/// auto-approved Requester whose ask approves it.
#[derive(Debug)]
struct NewQuality {
    quality: RequestedQuality,
    /// The User id of whoever changes it.
    by: i64,
}

/// Seasons of a Show to ask for, as the metadata source gave them.
#[derive(Debug)]
struct WantedSeasons {
    show_tmdb_id: i64,
    show_ids: ExternalIds,
    show_title: String,
    seasons: Vec<Season>,
    /// The Quality their new Requests take, unless the Show has one.
    quality: Option<RequestedQuality>,
    /// Whether the asker chose `quality`, as [`Wanted::quality_chosen`] says.
    quality_chosen: bool,
    /// The id of the Quality the asker named, if they did, which must be the Show's if it has one.
    chosen: Option<i64>,
}

/// Whose Requests `viewer` may see: everyone's (`None`), or only those they are a Requester of.
pub(crate) fn visible_to(viewer: &User) -> Option<i64> {
    (!viewer.permissions.allow(Permission::ViewRequests)).then_some(viewer.id)
}

impl Requests {
    /// The Requests in `db`, looking Movies up through `matcher`, publishing on `events` and
    /// handing Approved ones to the Fulfillers in `plugins`.
    pub(crate) fn new(
        db: Arc<Database>,
        matcher: Matcher,
        events: EventBus,
        plugins: Registry,
    ) -> Self {
        Self {
            contents: Contents::new(Arc::clone(&db)),
            hand_off: HandOff::new(Arc::clone(&db), events.clone(), plugins),
            db,
            matcher,
            events,
        }
    }

    /// Asks for the Movie with the TMDB id `tmdb_id` for `asker`, who joins its open Request, or
    /// opens one at the Quality `chosen`, else as [`Self::movie_quality`] picks: Approved, and handed
    /// to the Fulfiller for Movies, if `asker` may have Movies approved as they ask, else Pending.
    /// Such an `asker` joining a Pending Request approves it and hands it off the same way.
    /// Publishes what changed, if anything did.
    ///
    /// Refuses a `chosen` Quality from an `asker` who may not choose one, or that the Movie
    /// Fulfiller doesn't offer, even when joining.
    pub(crate) async fn create(
        &self,
        asker: &User,
        tmdb_id: i64,
        chosen: Option<i64>,
    ) -> Result<Request, Error> {
        // Refuses before asking TMDB, which may be down or slow; `ask` checks again for a scan
        // that Matched the Movie meanwhile.
        if self.contents.library_with_movie(tmdb_id).await?.is_some() {
            return Err(Error::InLibrary(tmdb_id));
        }
        let (quality, quality_chosen) = self.quality::<Movies>(asker, chosen).await?;
        // ponytail: looked up even to join an open Request, so the write has it if that Request
        // closes meanwhile. Look up only when opening one if TMDB's rate limit starts to bite.
        let movie = self.fetch(tmdb_id).await?;
        let wanted = Wanted {
            tmdb_id,
            season: None,
            title: movie.title,
            release_date: movie.release_date,
            show_ids: ExternalIds::default(),
            followed_show: None,
            quality,
            quality_chosen,
        };
        let state = opening_state(asker, Permission::AutoApproveMovie);
        let user = asker.id;
        let this = self.clone();
        // The write too, as in `create_seasons`.
        detached(async move {
            let (request, happened) = this
                .db
                .write(move |tx| Box::pin(ask_movie(tx, user, wanted, state)))
                .await?;
            // Only the asker who opens or approves the Request hands it off, so joining it can't
            // hand it twice.
            let hands_off = happened.iter().any(|event| handed_off_id(event).is_some());
            this.events.publish(happened);
            if hands_off && request.state == RequestState::Approved {
                return this.hand_off.request(request).await;
            }
            Ok(request)
        })
        .await
    }

    /// The Quality a new Request `asker` opens on the enabled Fulfiller of `Kind` takes, and
    /// whether they chose it, as [`qualities::for_request`] decides.
    async fn quality<Kind: 'static>(
        &self,
        asker: &User,
        chosen: Option<i64>,
    ) -> Result<(Option<RequestedQuality>, bool), Error> {
        let fulfiller = self.hand_off.fulfiller::<Kind>();
        let mut conn = self.db.reader().acquire().await?;
        Ok(qualities::for_request(&mut conn, asker, fulfiller.as_ref(), chosen).await??)
    }

    /// The Qualities of the enabled Fulfiller of `Kind`, in the order an admin listed them, with
    /// `asker`'s personal default among them if [`Self::quality`] would take it. Empty if no
    /// Fulfiller of `Kind` is enabled.
    pub(crate) async fn qualities<Kind: 'static>(
        &self,
        asker: &User,
    ) -> Result<(Vec<Quality>, Option<i64>), sqlx::Error> {
        let Some(fulfiller) = self.hand_off.fulfiller::<Kind>() else {
            return Ok((Vec::new(), None));
        };
        let mut conn = self.db.reader().acquire().await?;
        qualities::offered_to(&mut conn, asker, &fulfiller).await
    }

    /// `user`'s default Quality for each enabled Fulfiller, Movies first, whose list has more than
    /// one, or has one left after an admin removed `user`'s, to flag it.
    pub(crate) async fn personal_defaults(
        &self,
        user: i64,
    ) -> Result<Vec<DefaultQuality>, sqlx::Error> {
        let fulfillers = [
            (
                self.hand_off.fulfiller::<Movies>(),
                ExtensionPoint::EXTENSION_POINT_MOVIE_FULFILLER,
            ),
            (
                self.hand_off.fulfiller::<Shows>(),
                ExtensionPoint::EXTENSION_POINT_SHOW_FULFILLER,
            ),
        ];
        let mut conn = self.db.reader().acquire().await?;
        let mut defaults = Vec::new();
        for (plugin, fulfiller) in fulfillers {
            let Some(plugin) = plugin else {
                continue;
            };
            let qualities = qualities::list(&mut conn, &plugin).await?;
            let quality = qualities::personal_default(&mut conn, user, &plugin).await?;
            if qualities::shows_default(&qualities, quality) {
                defaults.push(DefaultQuality {
                    quality,
                    plugin_id: plugin.to_string(),
                    fulfiller: fulfiller.into(),
                    qualities,
                    ..Default::default()
                });
            }
        }
        Ok(defaults)
    }

    /// Sets `user`'s default Quality for the Fulfiller from the Plugin `plugin` to `quality`, or
    /// clears it. Refuses a Quality that isn't on its list.
    pub(crate) async fn set_personal_default(
        &self,
        user: i64,
        plugin: PluginId,
        quality: Option<i64>,
    ) -> Result<(), Error> {
        Ok(qualities::set_personal_default(&self.db, user, plugin, quality).await??)
    }

    /// The Qualities the enabled Fulfiller for titles of `kind` offers, none if there is no such
    /// Fulfiller.
    pub(crate) async fn offered_qualities(&self, kind: TitleKind) -> Result<Vec<Quality>, Error> {
        let fulfiller = match kind {
            TitleKind::Movie => self.hand_off.fulfiller::<Movies>(),
            TitleKind::Show => self.hand_off.fulfiller::<Shows>(),
        };
        let Some(fulfiller) = fulfiller else {
            return Ok(Vec::new());
        };
        let mut conn = self.db.reader().acquire().await?;
        Ok(qualities::list(&mut conn, &fulfiller).await?)
    }

    /// `manager` changing the Request `id`'s Quality to the one with the id `quality`, if given,
    /// which must be on the list of the Fulfiller the Request goes to.
    async fn new_quality(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<Option<NewQuality>, Error> {
        let Some(quality_id) = quality else {
            return Ok(None);
        };
        let request = load_one(&mut *self.db.reader().acquire().await?, id)
            .await?
            .ok_or(Error::NoSuchRequest(id))?;
        let fulfiller = self.hand_off.fulfiller_of(&request);
        self.offered_change(manager, fulfiller, quality_id)
            .await
            .map(Some)
    }

    /// `manager` changing a Quality to the one with the id `quality_id`, which must be on the list
    /// of the Fulfiller from the Plugin `fulfiller`.
    async fn offered_change(
        &self,
        manager: &User,
        fulfiller: Option<PluginId>,
        quality_id: i64,
    ) -> Result<NewQuality, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let quality = qualities::offered(&mut conn, fulfiller.as_ref(), quality_id).await??;
        Ok(NewQuality {
            quality,
            by: manager.id,
        })
    }

    /// Asks for the Seasons `numbers` of the Show with the TMDB id `tmdb_id` for `asker`, one
    /// Request per Season in Season order, each as [`Self::create`] asks for a Movie but with
    /// `AUTO_APPROVE_TV` and the Fulfiller for Shows. Asks for none if a Library has any of them:
    /// every aired Episode Matched. New Requests take the Show's Quality if it has one, refusing
    /// another `chosen` one, else as [`Self::create`] picks one.
    pub(crate) async fn create_seasons(
        &self,
        asker: &User,
        tmdb_id: i64,
        numbers: &BTreeSet<u32>,
        chosen: Option<i64>,
    ) -> Result<Vec<Request>, Error> {
        let (quality, quality_chosen) = self.quality::<Shows>(asker, chosen).await?;
        let id = ExternalId::Tmdb(tmdb_id);
        let (_, show) = self
            .matcher
            .show(&id)
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let mut seasons = Vec::with_capacity(numbers.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in numbers {
            let no_such_season = || Error::NoSuchSeason {
                show: tmdb_id,
                season: number,
            };
            if !show.seasons.contains(&number) {
                return Err(no_such_season());
            }
            let season = self
                .matcher
                .show_season(&id, number)
                .await
                .map_err(|err| looking_up(err, no_such_season()))?;
            seasons.push(season);
        }
        let wanted = WantedSeasons {
            show_tmdb_id: tmdb_id,
            show_ids: show.ids,
            show_title: show.title,
            seasons,
            quality,
            quality_chosen,
            chosen,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        let this = self.clone();
        // The write too, so a caller that stops waiting leaves no Season Approved behind.
        detached(async move {
            let (requests, happened) = this
                .db
                .write(move |tx| Box::pin(ask_seasons(tx, user, wanted, state)))
                .await?;
            let to_hand_off: HashSet<i64> = happened.iter().filter_map(handed_off_id).collect();
            this.events.publish(happened);
            this.hand_off.these(requests, &to_hand_off).await
        })
        .await
    }

    /// Asks for the whole Show with the TMDB id `tmdb_id` for `asker`: each current Season not in
    /// a Library as [`Self::create_seasons`] asks for it, at the Quality it picks, and later ones
    /// through the Show's open Followed Show, which `asker` joins or opens at that Quality, in the
    /// state their Season Requests open in. Returns the Followed Show and the Season Requests in
    /// Season order.
    ///
    /// Refuses a `chosen` Quality as [`Self::create_seasons`] does.
    pub(crate) async fn follow(
        &self,
        asker: &User,
        tmdb_id: i64,
        chosen: Option<i64>,
    ) -> Result<(FollowedShow, Vec<Request>), Error> {
        let (quality, quality_chosen) = self.quality::<Shows>(asker, chosen).await?;
        let id = ExternalId::Tmdb(tmdb_id);
        let (_, show) = self
            .matcher
            .show(&id)
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let wanted = WantedSeasons {
            show_tmdb_id: tmdb_id,
            seasons: self.seasons(tmdb_id, &show).await?,
            show_ids: show.ids,
            show_title: show.title,
            quality,
            quality_chosen,
            chosen,
        };
        let state = opening_state(asker, Permission::AutoApproveTv);
        let user = asker.id;
        let this = self.clone();
        // The write too, as in `create_seasons`.
        detached(async move {
            let (followed, requests, happened) = this
                .db
                .write(move |tx| Box::pin(follow(tx, user, wanted, state)))
                .await?;
            let to_hand_off: HashSet<i64> = happened.iter().filter_map(handed_off_id).collect();
            this.events.publish(happened);
            let requests = this.hand_off.these(requests, &to_hand_off).await?;
            Ok((followed, requests))
        })
        .await
    }

    /// Approves the Pending Followed Show `id` as `manager`, first changing its Show's Quality to
    /// the one with the id `quality` if given, and each Pending Season Request it opened, handing
    /// those to the Shows Fulfiller, publishing the Followed Show's change of state, then each
    /// Request's.
    pub(crate) async fn approve_followed(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<FollowedShow, Error> {
        let new_quality = match quality {
            Some(quality) => {
                let fulfiller = self.hand_off.fulfiller::<Shows>();
                Some(self.offered_change(manager, fulfiller, quality).await?)
            }
            None => None,
        };
        self.decide_followed(id, RequestState::Approved, None, new_quality)
            .await
    }

    /// Declines the Pending Followed Show `id`, and each Pending Season Request it opened, as
    /// [`Self::decline`] declines a Request.
    pub(crate) async fn decline_followed(
        &self,
        id: i64,
        reason: &str,
    ) -> Result<FollowedShow, Error> {
        self.decide_followed(id, RequestState::Declined, given_reason(reason), None)
            .await
    }

    async fn decide_followed(
        &self,
        id: i64,
        state: RequestState,
        reason: Option<String>,
        new_quality: Option<NewQuality>,
    ) -> Result<FollowedShow, Error> {
        let this = self.clone();
        // The write too, as in `approve`.
        detached(async move {
            let (followed, decided) = this
                .db
                .write(move |tx| Box::pin(decide_followed(tx, id, state, reason, new_quality)))
                .await?;
            let followed_changed = event(FollowedShowStateChanged {
                followed_show: followed.clone().into(),
                ..Default::default()
            });
            let seasons_changed = decided.iter().cloned().map(state_changed);
            this.events.publish(
                iter::once(followed_changed)
                    .chain(seasons_changed)
                    .collect(),
            );
            let ids: HashSet<i64> = decided.iter().map(|request| request.id).collect();
            this.hand_off.these(decided, &ids).await?;
            Ok(followed)
        })
        .await
    }

    /// Removes the Followed Show `id`, whatever its state, leaving the Season Requests it opened,
    /// publishes that it was cancelled, and has the Shows Fulfiller stop following the Show.
    pub(crate) async fn cancel_followed(&self, id: i64) -> Result<(), Error> {
        self.end_followed(move |tx| Box::pin(cancel_followed(tx, id)))
            .await
    }

    /// Takes `requester` off the Followed Show `id`, removing it once nobody is left on it, as
    /// [`Self::cancel_followed`] does, and publishes which of the two happened.
    pub(crate) async fn withdraw_followed(&self, requester: &User, id: i64) -> Result<(), Error> {
        let user = requester.id;
        let visible = visible_to(requester);
        self.end_followed(move |tx| Box::pin(withdraw_followed(tx, user, visible, id)))
            .await
    }

    /// Writes `end`, publishes the event it returns, then, if it also returns the Season Request
    /// of a Followed Show it ended, tells the Shows Fulfiller, if one is enabled, to stop
    /// following that Show. A failure to tell it is only logged: the Followed Show ended either
    /// way.
    ///
    /// Runs in a task of its own, so a caller that stops waiting leaves the Fulfiller following no
    /// ended Followed Show.
    async fn end_followed<F>(&self, end: F) -> Result<(), Error>
    where
        F: for<'t> FnOnce(
                &'t mut Transaction<'_, Sqlite>,
            ) -> BoxFuture<'t, Result<(Event, Option<Request>), Error>>
            + Send
            + 'static,
    {
        let this = self.clone();
        detached(async move {
            let (happened, followed_season) = this.db.write(end).await?;
            this.events.publish(vec![happened]);
            if let Some(followed_season) = followed_season {
                this.hand_off.unfollow(&followed_season).await;
            }
            Ok(())
        })
        .await
    }

    /// Approves the Pending Request `id` as `manager`, first changing its Quality to the one with
    /// the id `quality` if given, and hands it to its Fulfiller, publishing each change of its
    /// state.
    pub(crate) async fn approve(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<Request, Error> {
        let new_quality = self.new_quality(manager, id, quality).await?;
        let requests = self.clone();
        detached(async move {
            let approved = requests
                .decide(id, RequestState::Approved, None, new_quality)
                .await?;
            requests.hand_off.request(approved).await
        })
        .await
    }

    /// Moves the Failed Request `id` back to Approved as `manager`, changing its Quality as
    /// [`Self::approve`] does, and hands it to its Fulfiller again, publishing each change of its
    /// state.
    pub(crate) async fn retry(
        &self,
        manager: &User,
        id: i64,
        quality: Option<i64>,
    ) -> Result<Request, Error> {
        let new_quality = self.new_quality(manager, id, quality).await?;
        let requests = self.clone();
        detached(async move {
            let approved = requests
                .db
                .write(move |tx| Box::pin(retry(tx, id, new_quality)))
                .await?;
            requests
                .events
                .publish(vec![state_changed(approved.clone())]);
            requests.hand_off.request(approved).await
        })
        .await
    }

    /// Hands each Approved Request, oldest first, to its Fulfiller whenever a Plugin `came_up`
    /// names provides one. A failure is only logged, and leaves the rest waiting. Runs until
    /// `came_up` closes or it is dropped.
    pub(crate) async fn hand_off_as_fulfillers_come_up(
        &self,
        came_up: UnboundedReceiver<PluginId>,
    ) {
        self.hand_off.as_fulfillers_come_up(came_up).await;
    }

    /// Asks the Fulfillers how their Processing Requests are progressing, every `interval` while any
    /// Request is Processing, and at once when one becomes Processing. Runs until dropped.
    pub(crate) async fn follow_progress(&self, interval: Duration) {
        self.hand_off.follow_progress(interval).await;
    }

    /// Declines the Pending Request `id`, giving its Requesters `reason`, trimmed, unless it is
    /// blank, and publishes that its state changed.
    pub(crate) async fn decline(&self, id: i64, reason: &str) -> Result<Request, Error> {
        let reason = given_reason(reason);
        let this = self.clone();
        // The write commits even if the caller stops waiting, so its event must follow (ADR 0029).
        detached(async move { this.decide(id, RequestState::Declined, reason, None).await }).await
    }

    async fn decide(
        &self,
        id: i64,
        state: RequestState,
        reason: Option<String>,
        new_quality: Option<NewQuality>,
    ) -> Result<Request, Error> {
        let request = self
            .db
            .write(move |tx| {
                Box::pin(async move {
                    // Undone with the rest if the Request can't be decided.
                    change_quality(tx, id, new_quality).await?;
                    decide(tx, id, state, reason).await
                })
            })
            .await?;
        self.events.publish(vec![state_changed(request.clone())]);
        Ok(request)
    }

    /// Removes the Request `id`, whatever its state, and publishes that it was cancelled.
    pub(crate) async fn cancel(&self, id: i64) -> Result<(), Error> {
        let this = self.clone();
        // As in `decline`.
        detached(async move {
            let request = this.db.write(move |tx| Box::pin(cancel(tx, id))).await?;
            this.events.publish(vec![event(RequestCancelled {
                request: request.into(),
                ..Default::default()
            })]);
            Ok(())
        })
        .await
    }

    /// Takes `requester` off the Pending Request `id`, removing it once nobody is left on it, and
    /// publishes which of the two happened.
    pub(crate) async fn withdraw(&self, requester: &User, id: i64) -> Result<(), Error> {
        let user = requester.id;
        let visible = visible_to(requester);
        let this = self.clone();
        // As in `decline`.
        detached(async move {
            let happened = this
                .db
                .write(move |tx| Box::pin(withdraw(tx, user, visible, id)))
                .await?;
            this.events.publish(vec![happened]);
            Ok(())
        })
        .await
    }

    /// A page of at most `size` of the Requests and Followed Shows `viewer` may see, newest first:
    /// only those in `state` if given, each matched on its own. `token` is the page before's
    /// `next_page_token`, or empty for the first page.
    pub(crate) async fn list(
        &self,
        viewer: &User,
        state: Option<RequestState>,
        size: usize,
        token: &str,
    ) -> Result<ListRequestsResponse, Error> {
        let after = read_page_token(token, state)?;
        let mut conn = self.db.reader().acquire().await?;
        // ponytail: reads every row `viewer` may see to list one page; page in the queries once a
        // Server's Requests number in the thousands.
        let requests = load(&mut conn, None, visible_to(viewer)).await?;
        let followed = load_followed(&mut conn, None, visible_to(viewer)).await?;
        let mut rows: Vec<Listed> = requests
            .into_iter()
            .map(Listed::Request)
            .chain(followed.into_iter().map(Listed::Followed))
            .filter(|row| state.is_none_or(|state| row.state() == state))
            .filter(|row| after.is_none_or(|after| row.place() < after))
            .collect();
        rows.sort_unstable_by_key(|row| Reverse(row.place()));
        let more = rows.len() > size;
        rows.truncate(size);
        let next_page_token = match rows.last() {
            Some(last) if more => page_token(state, last.place()),
            _ => String::new(),
        };
        let mut page = ListRequestsResponse {
            next_page_token,
            ..Default::default()
        };
        for row in rows {
            match row {
                Listed::Request(request) => {
                    page.requests.push(self.hand_off.with_progress(request));
                }
                Listed::Followed(show) => page.followed_shows.push(show),
            }
        }
        Ok(page)
    }

    /// How many of the Requests and Followed Shows `viewer` may see are Pending, as many as
    /// [`Self::list`] shows them, counting a Followed Show apart from its Season Requests.
    pub(crate) async fn count_pending(&self, viewer: &User) -> Result<u32, sqlx::Error> {
        let requester = visible_to(viewer);
        // Keep the visibility checks in step with `load` and `load_followed`, or the count and the
        // Pending filter disagree.
        sqlx::query_scalar!(
            r#"SELECT (SELECT COUNT(*) FROM requests
                       WHERE state = 'pending'
                         AND (?1 IS NULL OR EXISTS (SELECT 1 FROM requesters
                                                    WHERE request_id = requests.id
                                                      AND user_id = ?1)))
                + (SELECT COUNT(*) FROM followed_shows
                   WHERE state = 'pending'
                     AND (?1 IS NULL OR EXISTS (SELECT 1 FROM followed_show_requesters
                                                WHERE followed_show_id = followed_shows.id
                                                  AND user_id = ?1))) AS "count!: u32""#,
            requester
        )
        .fetch_one(self.db.reader())
        .await
    }

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
        for (tmdb_id, plugin, candidate) in candidates {
            let mut movie = look_around(&mut conn, viewer, tmdb_id, plugin, candidate).await?;
            movie.request = movie
                .request
                .map(|request| self.hand_off.with_progress(request));
            found.push(movie);
        }
        Ok(found)
    }

    /// The Shows the metadata source finds titled like `title`, best fit first.
    pub(crate) async fn search_shows(
        &self,
        title: String,
    ) -> Result<Vec<FoundShow<Candidate>>, Error> {
        let candidates = self.find(TitleKind::Show, title).await?;
        Ok(candidates
            .into_iter()
            .map(|(tmdb_id, plugin, show)| FoundShow {
                tmdb_id,
                plugin,
                show,
                seasons: Vec::new(),
                followed_show: None,
                quality: None,
            })
            .collect())
    }

    /// The titles of `kind` the metadata source finds titled like `title`, best fit first, each
    /// with its TMDB id and the Plugin whose Provider found it.
    async fn find(
        &self,
        kind: TitleKind,
        title: String,
    ) -> Result<Vec<(i64, PluginId, Candidate)>, Error> {
        let query = SearchQuery {
            kind,
            title,
            year: None,
        };
        let candidates = self
            .matcher
            .search_metadata(&query)
            .await
            .map_err(|err| match err {
                ManualError::Database(err) => Error::Database(err),
                // A search names no item or title, so it fails only when no Provider answers.
                ManualError::NoSuchItem(_)
                | ManualError::NoSuchTitle(_)
                | ManualError::Unavailable => Error::Unavailable,
            })?;
        // Requests name titles by TMDB id, so a title without one can't be asked for.
        Ok(candidates
            .into_iter()
            .filter_map(|(plugin, candidate)| match candidate.id {
                ExternalId::Tmdb(tmdb_id) => Some((tmdb_id, plugin, candidate)),
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
    ) -> Result<FoundShow<Show>, Error> {
        let (plugin, show) = self
            .matcher
            .show(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchShow(tmdb_id)))?;
        let seasons = self.seasons(tmdb_id, &show).await?;
        let mut conn = self.db.reader().acquire().await?;
        let mut seasons = look_around_seasons(&mut conn, viewer, tmdb_id, seasons).await?;
        for season in &mut seasons {
            let request = season.request.take();
            season.request = request
                .map(|request| self.hand_off.with_progress(request))
                .into();
        }
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
    async fn seasons(&self, tmdb_id: i64, show: &Show) -> Result<Vec<Season>, Error> {
        let id = ExternalId::Tmdb(tmdb_id);
        let mut seasons = Vec::with_capacity(show.seasons.len());
        // ponytail: one metadata lookup per Season, one after another.
        for &number in &show.seasons {
            let season = self.matcher.show_season(&id, number).await;
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
        let mut found = look_around(&mut conn, viewer, tmdb_id, plugin, movie).await?;
        found.request = found
            .request
            .map(|request| self.hand_off.with_progress(request));
        Ok(found)
    }

    async fn fetch(&self, tmdb_id: i64) -> Result<Metadata, Error> {
        self.matcher
            .movie(&ExternalId::Tmdb(tmdb_id))
            .await
            .map_err(|err| looking_up(err, Error::NoSuchMovie(tmdb_id)))
    }

    /// The Request `id`, if `viewer` may see it.
    pub(crate) async fn get(&self, viewer: &User, id: i64) -> Result<Request, Error> {
        let mut conn = self.db.reader().acquire().await?;
        let found = load(&mut conn, Some(id), visible_to(viewer)).await?;
        let request = found.into_iter().next().ok_or(Error::NoSuchRequest(id))?;
        Ok(self.hand_off.with_progress(request))
    }
}

/// A row of [`Requests::list`].
#[expect(
    clippy::large_enum_variant,
    reason = "most rows are Requests, so boxing them would save nothing"
)]
enum Listed {
    Request(Request),
    Followed(FollowedShow),
}

impl Listed {
    fn state(&self) -> RequestState {
        match self {
            Self::Request(request) => request.state,
            Self::Followed(followed) => followed.state,
        }
        .as_known()
        .unwrap_or_default()
    }

    fn place(&self) -> Place {
        match self {
            Self::Request(request) => Place {
                created_at: request.created_at,
                is_request: true,
                id: request.id,
            },
            Self::Followed(followed) => Place {
                created_at: followed.created_at,
                is_request: false,
                id: followed.id,
            },
        }
    }
}

/// Where a row sits in [`Requests::list`]'s order, newer sorting greater. A Request sorts as
/// newer than a Followed Show opened in the same second, so it lists first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Place {
    created_at: i64,
    is_request: bool,
    id: i64,
}

/// The token for the page after the one ending at `last`, tied to the filter `state`. Encoded so
/// clients treat it as opaque and don't build their own.
#[expect(
    clippy::expect_used,
    reason = "a string and plain numbers always serialize to JSON"
)]
fn page_token(state: Option<RequestState>, last: Place) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&(state.map(stored), last)).expect("serializes"))
}

/// Where the page `token` continues from, or `None` for the first page.
fn read_page_token(token: &str, state: Option<RequestState>) -> Result<Option<Place>, Error> {
    if token.is_empty() {
        return Ok(None);
    }
    let json = URL_SAFE_NO_PAD.decode(token).unwrap_or_default();
    match serde_json::from_slice::<(Option<String>, Place)>(&json) {
        Ok((filter, last)) if filter.as_deref() == state.map(stored) => Ok(Some(last)),
        _ => Err(Error::PageTokenInvalid),
    }
}

/// `err` from looking a title up, which is `missing` if the metadata source doesn't have it.
fn looking_up(err: ManualError, missing: Error) -> Error {
    match err {
        // A look-up names no item, so its only refusal is the title's.
        ManualError::NoSuchTitle(_) | ManualError::NoSuchItem(_) => missing,
        ManualError::Unavailable => Error::Unavailable,
        ManualError::Database(err) => Error::Database(err),
    }
}

/// `reason`, trimmed, unless it is blank.
fn given_reason(reason: &str) -> Option<String> {
    let reason = reason.trim();
    (!reason.is_empty()).then(|| reason.to_owned())
}

/// The state `asker`'s new Request opens in: Approved if they pass a check for `auto_approve`,
/// else Pending.
fn opening_state(asker: &User, auto_approve: Permission) -> RequestState {
    if asker.permissions.allow(auto_approve) {
        RequestState::Approved
    } else {
        RequestState::Pending
    }
}

/// `movie`, the Movie `tmdb_id` as `plugin`'s Provider gave it, with the Library that has it and
/// its open Request, if `viewer` may see it.
async fn look_around<M>(
    conn: &mut SqliteConnection,
    viewer: &User,
    tmdb_id: i64,
    plugin: PluginId,
    movie: M,
) -> Result<Found<M>, sqlx::Error> {
    let library = contents::library_with_movie(conn, tmdb_id).await?;
    let request = visible_open_request(conn, viewer, tmdb_id, None).await?;
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
    conn: &mut SqliteConnection,
    viewer: &User,
    show_tmdb_id: i64,
    seasons: Vec<Season>,
) -> Result<Vec<FoundSeason>, sqlx::Error> {
    let today = today(conn).await?;
    let mut found = Vec::with_capacity(seasons.len());
    // ponytail: two or three queries per Season, and a Show has a few dozen at most.
    for season in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        let request = visible_open_request(conn, viewer, show_tmdb_id, Some(season.number)).await?;
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

/// The open Request for the Movie `tmdb_id`, or for its Season `season` if `tmdb_id` is a Show's,
/// if `viewer` may see it.
async fn visible_open_request(
    conn: &mut SqliteConnection,
    viewer: &User,
    tmdb_id: i64,
    season: Option<u32>,
) -> Result<Option<Request>, sqlx::Error> {
    let Some(id) = open_request(conn, tmdb_id, season).await? else {
        return Ok(None);
    };
    let visible = load(conn, Some(id), visible_to(viewer)).await?;
    Ok(visible.into_iter().next())
}

/// Today as `YYYY-MM-DD`, by SQLite's calendar, since `std` has none.
async fn today(conn: &mut SqliteConnection) -> Result<String, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT date('now') AS "today!: String""#)
        .fetch_one(&mut *conn)
        .await
}

/// [`ask_approving`]s for the Movie `wanted`, unless a Library has it.
async fn ask_movie(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<(Request, Vec<Event>), Error> {
    if contents::library_with_movie(conn, wanted.tmdb_id)
        .await?
        .is_some()
    {
        return Err(Error::InLibrary(wanted.tmdb_id));
    }
    ask_approving(conn, user, wanted, state).await
}

/// [`ask_approving`]s for each of the Seasons `show`, unless a Library has any of them. Returns the
/// Requests in the order of `show.seasons`, and the events for what changed.
async fn ask_seasons(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Result<(Vec<Request>, Vec<Event>), Error> {
    // Read in the write, so two asks for the Show's Seasons at once can't take two Qualities.
    let (quality, quality_chosen) = show_requests(conn, show.show_tmdb_id)
        .await?
        .for_seasons((show.quality.clone(), show.quality_chosen), show.chosen)?;
    let WantedSeasons {
        show_tmdb_id,
        show_ids,
        show_title,
        seasons,
        ..
    } = show;
    let today = today(conn).await?;
    let mut requests = Vec::with_capacity(seasons.len());
    let mut happened = Vec::new();
    for season in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        if availability(&season.episodes, &matched, &today) == Some(RequestState::Available) {
            return Err(Error::SeasonInLibrary {
                show: show_tmdb_id,
                season: season.number,
            });
        }
        let wanted = Wanted {
            tmdb_id: show_tmdb_id,
            season: Some(season.number),
            title: show_title.clone(),
            release_date: season.air_date,
            show_ids: show_ids.clone(),
            followed_show: None,
            quality: quality.clone(),
            quality_chosen,
        };
        let (request, events) = ask_approving(conn, user, wanted, state).await?;
        requests.push(request);
        happened.extend(events);
    }
    Ok((requests, happened))
}

/// [`ask`]s for `wanted`, then approves the open Request `user` joined if it is Pending and `state`
/// is Approved, since their ask would have opened it Approved, at the Quality
/// [`qualities::joiner_approves_at`] gives.
async fn ask_approving(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<(Request, Vec<Event>), Error> {
    let new_quality = qualities::joiner_approves_at(wanted.quality.clone(), wanted.quality_chosen)
        .map(|quality| NewQuality { quality, by: user });
    let (request, happened) = ask(conn, user, wanted, state).await?;
    let mut happened = Vec::from_iter(happened);
    if state != RequestState::Approved || request.state != RequestState::Pending {
        return Ok((request, happened));
    }
    change_quality(conn, request.id, new_quality).await?;
    let approved = decide(conn, request.id, RequestState::Approved, None).await?;
    happened.push(state_changed(approved.clone()));
    Ok((approved, happened))
}

/// How much a Library has of the Season with `episodes`, of which those numbered in `matched` are
/// Matched, counting only those aired by `today` (`YYYY-MM-DD`, as air dates are, so they sort as
/// text): Available once every aired Episode is Matched, Partially Available while some are, and
/// `None` while none are. So a Season yet to air isn't in a Library, and can be Requested.
fn availability(episodes: &[Episode], matched: &HashSet<i64>, today: &str) -> Option<RequestState> {
    let aired: Vec<i64> = episodes
        .iter()
        .filter(|episode| {
            episode
                .air_date
                .as_deref()
                .is_some_and(|aired| aired <= today)
        })
        .map(|episode| episode.number.into())
        .collect();
    let have = aired
        .iter()
        .filter(|number| matched.contains(number))
        .count();
    if have == 0 {
        None
    } else if have == aired.len() {
        Some(RequestState::Available)
    } else {
        Some(RequestState::PartiallyAvailable)
    }
}

/// Opens a Request for `wanted` in `state`, as stored, with no Requesters yet. Returns its ID.
async fn insert_request(
    conn: &mut SqliteConnection,
    wanted: &Wanted,
    state: &str,
    now: i64,
) -> Result<i64, sqlx::Error> {
    let (quality_id, quality_name, quality_profile_id) = quality_columns(wanted.quality.as_ref());
    sqlx::query_scalar!(
        "INSERT INTO requests (tmdb_id, season, title, release_date, show_imdb_id, show_tvdb_id,
                               state, created_at, followed_show_id, quality_id, quality_name,
                               quality_profile_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
        wanted.tmdb_id,
        wanted.season,
        wanted.title,
        wanted.release_date,
        wanted.show_ids.imdb,
        wanted.show_ids.tvdb,
        state,
        now,
        wanted.followed_show,
        quality_id,
        quality_name,
        quality_profile_id
    )
    .fetch_one(&mut *conn)
    .await
}

/// Adds `user` as a Requester of the open Request for `wanted`, opening one in `state` if there
/// is none. Returns the Request, and the event for what changed, if anything did.
async fn ask(
    conn: &mut SqliteConnection,
    user: i64,
    wanted: Wanted,
    state: RequestState,
) -> Result<(Request, Option<Event>), sqlx::Error> {
    let now = now();
    let state = stored(state);
    let open = open_request(conn, wanted.tmdb_id, wanted.season).await?;
    let opened = open.is_none();
    let id = match open {
        Some(id) => id,
        None => insert_request(conn, &wanted, state, now).await?,
    };
    let joined = sqlx::query!(
        "INSERT INTO requesters (request_id, user_id, requested_at) VALUES (?, ?, ?)
         ON CONFLICT DO NOTHING",
        id,
        user,
        now
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    let happened = if opened {
        Some(event(RequestCreated {
            request: request.clone().into(),
            ..Default::default()
        }))
    } else if joined {
        Some(event(RequesterAdded {
            request: request.clone().into(),
            user_id: user,
            ..Default::default()
        }))
    } else {
        None
    };
    Ok((request, happened))
}

/// Makes Available the open Request for each of the Movies `tmdb_ids` that a Library now has,
/// and returns an event for each (ADR 0012). Declined Requests are closed, so they stay as they
/// are.
pub(crate) async fn make_available(
    conn: &mut SqliteConnection,
    tmdb_ids: Vec<i64>,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut happened = Vec::new();
    for tmdb_id in tmdb_ids {
        let Some(id) = open_request(conn, tmdb_id, None).await? else {
            continue;
        };
        // Not every Matched id is a Movie in a Library: the Match isn't stored if its Item went
        // since it was decided about, and an Episode's TMDB id can equal an unrelated Movie's.
        if contents::library_with_movie(conn, tmdb_id).await?.is_none() {
            continue;
        }
        happened.extend(change_state(conn, id, RequestState::Available).await?);
    }
    Ok(happened)
}

/// Makes the open Request for each of `seasons`, as the metadata source gave them just now with
/// their Show's TMDB id, Partially Available or Available by how many of its aired Episodes a
/// Library has, and returns an event for each Request that changed (ADR 0012). Declined and
/// Available Requests are closed, so Episodes that air later don't reopen them. A Season that came
/// out after the Show's Approved Followed Show was opened gets a Request in that state.
pub(crate) async fn make_seasons_available(
    conn: &mut SqliteConnection,
    seasons: Vec<(i64, Season)>,
) -> Result<Vec<Event>, sqlx::Error> {
    let today = today(conn).await?;
    let mut happened = Vec::new();
    for (show_tmdb_id, season) in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        let Some(state) = availability(&season.episodes, &matched, &today) else {
            continue;
        };
        let changed = match open_request(conn, show_tmdb_id, Some(season.number)).await? {
            Some(id) => change_state(conn, id, state).await?,
            None => open_followed_season(conn, show_tmdb_id, &season, state).await?,
        };
        happened.extend(changed);
    }
    Ok(happened)
}

/// Opens a Request in `state` for `season` of the Show `show_tmdb_id`, which has no open one, for
/// the Requesters of the Show's Approved Followed Show, if the Season is numbered after every one
/// the Followed Show covers and has no Available Request, so a Season once Available isn't asked
/// for again. The Followed Show covers the Season from then on, so one it opened and someone
/// declined, cancelled or withdrew from isn't opened again. Returns the event for it.
async fn open_followed_season(
    conn: &mut SqliteConnection,
    show_tmdb_id: i64,
    season: &Season,
    state: RequestState,
) -> Result<Option<Event>, sqlx::Error> {
    // ponytail: a Season that came out but was skipped, such as a Season 0 added later, gets no
    // Request; ask for it on its own.
    let Some(followed) = sqlx::query!(
        r#"SELECT id AS "id!", title FROM followed_shows
           WHERE tmdb_id = ? AND state = 'approved'
             AND (latest_season IS NULL OR latest_season < ?)"#,
        show_tmdb_id,
        season.number
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let available = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM requests
                          WHERE tmdb_id = ? AND season = ? AND state = 'available') AS "is!: bool""#,
        show_tmdb_id,
        season.number
    )
    .fetch_one(&mut *conn)
    .await?;
    if available {
        return Ok(None);
    }
    let wanted = Wanted {
        tmdb_id: show_tmdb_id,
        season: Some(season.number),
        title: followed.title,
        release_date: season.air_date.clone(),
        // ponytail: a Followed Show stores no Show ids, and this Season opens already in a Library.
        show_ids: ExternalIds::default(),
        followed_show: Some(followed.id),
        quality: show_requests(conn, show_tmdb_id).await?.quality().cloned(),
        quality_chosen: false,
    };
    let id = insert_request(conn, &wanted, stored(state), now()).await?;
    sqlx::query!(
        "UPDATE followed_shows SET latest_season = ? WHERE id = ?",
        season.number,
        followed.id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "INSERT INTO requesters (request_id, user_id, requested_at)
         SELECT ?, user_id, requested_at FROM followed_show_requesters WHERE followed_show_id = ?
         ORDER BY requested_at, rowid",
        id,
        followed.id
    )
    .execute(&mut *conn)
    .await?;
    let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    Ok(Some(event(RequestCreated {
        request: request.into(),
        ..Default::default()
    })))
}

/// Moves the Request `id` to `state`, and returns the event for it, or `None` if it was in
/// `state` already.
async fn change_state(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
) -> Result<Option<Event>, sqlx::Error> {
    let state = stored(state);
    let changed = sqlx::query!(
        "UPDATE requests SET state = ?1, failure_reason = NULL WHERE id = ?2 AND state != ?1",
        state,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if !changed {
        return Ok(None);
    }
    let request = load_one(conn, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    Ok(Some(event(RequestStateChanged {
        request: request.into(),
        ..Default::default()
    })))
}

/// The open Request for the Movie `tmdb_id`, or for its Season `season` if `tmdb_id` is a Show's.
async fn open_request(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
    season: Option<u32>,
) -> Result<Option<i64>, sqlx::Error> {
    // Open as `one_open_request_per_movie_or_season` has it.
    sqlx::query_scalar!(
        "SELECT id FROM requests
         WHERE tmdb_id = ? AND season IS ? AND state NOT IN ('declined', 'available')",
        tmdb_id,
        season
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Moves the Pending Request `id` to `state` with `reason`, and returns it as it is now.
async fn decide(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
    reason: Option<String>,
) -> Result<Request, Error> {
    let state = stored(state);
    let decided = sqlx::query!(
        "UPDATE requests SET state = ?, decline_reason = ? WHERE id = ? AND state = 'pending'",
        state,
        reason,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    if !decided {
        return Err(Error::NotPending(id));
    }
    Ok(request)
}

/// Changes the Request `id`'s Quality to `new_quality`, if given, as [`requality`] does.
async fn change_quality(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<(), Error> {
    let Some(new_quality) = new_quality else {
        return Ok(());
    };
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    let show = request
        .movie
        .is_unset()
        .then_some(request.season.show_tmdb_id);
    requality(conn, Some(id), show, new_quality).await
}

/// Changes the Quality of the Followed Show `id`'s Show to `new_quality`, if given, as
/// [`requality`] does.
async fn change_followed_quality(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<(), Error> {
    let Some(new_quality) = new_quality else {
        return Ok(());
    };
    let show = sqlx::query_scalar!("SELECT tmdb_id FROM followed_shows WHERE id = ?", id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(Error::NoSuchFollowedShow(id))?;
    requality(conn, None, Some(show), new_quality).await
}

/// Changes the Quality of the Request `id`, if given, and of the Show `show`, if given, to
/// `quality`, where it isn't that one, keeping each change as that Request's latest in place of
/// any earlier one. What the Show's change moves, or why it is refused, is
/// [`qualities::ShowRequests::change`]'s to say.
async fn requality(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    show: Option<i64>,
    NewQuality { quality, by }: NewQuality,
) -> Result<(), Error> {
    let (seasons, followed) = match show {
        Some(show) => {
            let moves = show_requests(conn, show).await?.change(&quality)?;
            (moves.seasons, moves.followed)
        }
        None => (Vec::new(), false),
    };
    let at = now();
    // ponytail: the Show's other Season Requests change without an event, so a client sees it on
    // its next read; publish one for each if a screen shows them live.
    for request in id.into_iter().chain(seasons) {
        // SQLite reads `quality_name` on the right as it was before this UPDATE.
        sqlx::query!(
            "UPDATE requests
             SET quality_id = ?1, quality_name = ?2, quality_profile_id = ?3,
                 quality_changed_by = ?4, quality_changed_from = quality_name,
                 quality_changed_at = ?5
             WHERE id = ?6 AND quality_id IS NOT ?1",
            quality.id,
            quality.name,
            quality.profile_id,
            by,
            at,
            request
        )
        .execute(&mut *conn)
        .await?;
    }
    if followed {
        // Open as `one_open_followed_show` has it.
        sqlx::query!(
            "UPDATE followed_shows
             SET quality_id = ?, quality_name = ?, quality_profile_id = ?
             WHERE tmdb_id = ? AND state != 'declined'",
            quality.id,
            quality.name,
            quality.profile_id,
            show
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The Season Requests, in the order they were made, and the open Followed Show of the Show
/// `show_tmdb_id`, for its Quality rules.
async fn show_requests(
    conn: &mut SqliteConnection,
    show_tmdb_id: i64,
) -> Result<qualities::ShowRequests, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", state, fulfiller_reference IS NOT NULL AS "handed_off!: bool",
                  quality_id, quality_name, quality_profile_id,
                  quality_used_id, quality_used_name, quality_used_profile_id
           FROM requests WHERE tmdb_id = ? AND season IS NOT NULL
           ORDER BY created_at, id"#,
        show_tmdb_id
    )
    .fetch_all(&mut *conn)
    .await?;
    let seasons = rows
        .into_iter()
        .map(|row| qualities::SeasonRequest {
            id: row.id,
            asked: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id),
            used: stored_quality(
                row.quality_used_id,
                row.quality_used_name,
                row.quality_used_profile_id,
            ),
            state: read_state(&row.state),
            handed_off: row.handed_off,
        })
        .collect();
    // Open as `one_open_followed_show` has it.
    let followed = sqlx::query!(
        "SELECT quality_id, quality_name, quality_profile_id FROM followed_shows
         WHERE tmdb_id = ? AND state != 'declined'",
        show_tmdb_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(qualities::ShowRequests {
        seasons,
        followed: followed.is_some(),
        followed_quality: followed.and_then(|row| {
            stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
        }),
    })
}

/// Moves the Approved Request `id`, asked at the Quality `asked`, on as its Fulfiller, from the
/// Plugin `plugin`, said: to Processing, keeping its reference and the Quality used, or to Failed,
/// keeping why. Returns it as it is now, or `None` if it isn't Approved any more.
async fn settle(
    conn: &mut SqliteConnection,
    id: i64,
    fulfilled: Result<Fulfilled, String>,
    asked: Option<RequestedQuality>,
    plugin: Option<PluginId>,
) -> Result<Option<Request>, sqlx::Error> {
    // In the same write as the reference, so a failed read can't lose a Request the Fulfiller took.
    let (state, reference, used, reason) = match fulfilled {
        Ok(fulfilled) => {
            let listed = match &plugin {
                Some(plugin) => qualities::list(conn, plugin).await?,
                None => Vec::new(),
            };
            let used = qualities::used(asked.as_ref(), &listed, &fulfilled);
            (
                RequestState::Processing,
                Some(fulfilled.reference),
                used,
                None,
            )
        }
        Err(reason) => (RequestState::Failed, None, None, Some(reason)),
    };
    let state = stored(state);
    let (used_id, used_name, used_profile_id) = quality_columns(used.as_ref());
    let settled = sqlx::query!(
        "UPDATE requests SET state = ?, fulfiller_reference = ?, failure_reason = ?,
                             quality_used_id = ?, quality_used_name = ?, quality_used_profile_id = ?
         WHERE id = ? AND state = 'approved'",
        state,
        reference,
        reason,
        used_id,
        used_name,
        used_profile_id,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    if !settled {
        return Ok(None);
    }
    load_one(conn, id).await
}

/// Moves the Failed Request `id` back to Approved at `new_quality`, if given, forgetting why it
/// failed, and returns it as it is now.
async fn retry(
    conn: &mut SqliteConnection,
    id: i64,
    new_quality: Option<NewQuality>,
) -> Result<Request, Error> {
    // Undone with the rest if the Request isn't Failed.
    change_quality(conn, id, new_quality).await?;
    let retried = sqlx::query!(
        "UPDATE requests SET state = 'approved', failure_reason = NULL
         WHERE id = ? AND state = 'failed'",
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    if !retried {
        return Err(Error::NotFailed(id));
    }
    Ok(request)
}

/// Removes the Request `id`, and its Requesters with it, and returns it as it was.
async fn cancel(conn: &mut SqliteConnection, id: i64) -> Result<Request, Error> {
    let request = load_one(conn, id).await?.ok_or(Error::NoSuchRequest(id))?;
    sqlx::query!("DELETE FROM requests WHERE id = ?", id)
        .execute(&mut *conn)
        .await?;
    Ok(request)
}

/// Takes `user` off the Pending Request `id`, among those `visible` lets them see, removing the
/// Request if they were its last Requester. Returns the event for what changed.
async fn withdraw(
    conn: &mut SqliteConnection,
    user: i64,
    visible: Option<i64>,
    id: i64,
) -> Result<Event, Error> {
    let request = load(conn, Some(id), visible)
        .await?
        .into_iter()
        .next()
        .ok_or(Error::NoSuchRequest(id))?;
    if !request.requesters.iter().any(|r| r.user_id == user) {
        return Err(Error::NotARequester(id));
    }
    if request.state != RequestState::Pending {
        return Err(Error::NotPending(id));
    }
    if request.requesters.len() == 1 {
        sqlx::query!("DELETE FROM requests WHERE id = ?", id)
            .execute(&mut *conn)
            .await?;
        return Ok(event(RequestWithdrawn {
            request: request.into(),
            ..Default::default()
        }));
    }
    sqlx::query!(
        "DELETE FROM requesters WHERE request_id = ? AND user_id = ?",
        id,
        user
    )
    .execute(&mut *conn)
    .await?;
    Ok(event(RequesterRemoved {
        request: request.into(),
        user_id: user,
        ..Default::default()
    }))
}

/// Adds `user` as a Requester of the open Followed Show for `show`, opening one in `state` if there
/// is none, then [`ask`]s for each of its Seasons not in a Library, opening missing ones for the
/// Followed Show in its state, and [`ask_approving`]s instead if `user` may auto-approve. Returns
/// the Followed Show, the Season Requests and the events for what changed, the Followed Show's first.
async fn follow(
    conn: &mut SqliteConnection,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Result<(FollowedShow, Vec<Request>, Vec<Event>), Error> {
    let now = now();
    // Read in the write, so two asks for the Show's Seasons at once can't take two Qualities.
    let (quality, quality_chosen) = show_requests(conn, show.show_tmdb_id)
        .await?
        .for_seasons((show.quality.clone(), show.quality_chosen), show.chosen)?;
    let already_open = open_followed_show(conn, show.show_tmdb_id).await?;
    let id = if let Some(id) = already_open {
        id
    } else {
        open_followed(conn, &show, quality.as_ref(), state, now).await?
    };
    let WantedSeasons {
        show_tmdb_id,
        show_ids,
        show_title,
        seasons,
        ..
    } = show;
    let joined = sqlx::query!(
        "INSERT INTO followed_show_requesters (followed_show_id, user_id, requested_at) VALUES (?, ?, ?)
         ON CONFLICT DO NOTHING",
        id,
        user,
        now
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(sqlx::Error::RowNotFound)?;
    let mut happened = Vec::new();
    if already_open.is_none() {
        happened.push(event(FollowedShowCreated {
            followed_show: followed.clone().into(),
            ..Default::default()
        }));
    } else if joined {
        happened.push(event(FollowedShowRequesterAdded {
            followed_show: followed.clone().into(),
            user_id: user,
            ..Default::default()
        }));
    }
    let may_auto_approve = state == RequestState::Approved;
    // An Approved Followed Show was approved once for every Season, so needs no approving again.
    let state = followed.state.as_known().unwrap_or(state);
    let today = today(conn).await?;
    let mut requests = Vec::with_capacity(seasons.len());
    for season in seasons {
        let matched = contents::matched_episodes(conn, show_tmdb_id, season.number.into()).await?;
        if availability(&season.episodes, &matched, &today) == Some(RequestState::Available) {
            continue;
        }
        let wanted = Wanted {
            tmdb_id: show_tmdb_id,
            season: Some(season.number),
            title: show_title.clone(),
            release_date: season.air_date,
            show_ids: show_ids.clone(),
            followed_show: Some(id),
            quality: quality.clone(),
            quality_chosen,
        };
        // Only `user`'s own auto-approve approves someone else's Pending Season Request: joining an
        // Approved Followed Show doesn't.
        let (request, events) = if may_auto_approve {
            ask_approving(conn, user, wanted, state).await?
        } else {
            let (request, event) = ask(conn, user, wanted, state).await?;
            (request, Vec::from_iter(event))
        };
        requests.push(request);
        happened.extend(events);
    }
    Ok((followed, requests, happened))
}

/// Opens a Followed Show of `show` at `quality` in `state`, returning its id.
async fn open_followed(
    conn: &mut SqliteConnection,
    show: &WantedSeasons,
    quality: Option<&RequestedQuality>,
    state: RequestState,
    now: i64,
) -> Result<i64, sqlx::Error> {
    let (quality_id, quality_name, quality_profile_id) = quality_columns(quality);
    sqlx::query_scalar!(
        "INSERT INTO followed_shows (tmdb_id, title, state, latest_season, created_at, quality_id,
                                     quality_name, quality_profile_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
        show.show_tmdb_id,
        show.show_title,
        stored(state),
        show.seasons.iter().map(|season| season.number).max(),
        now,
        quality_id,
        quality_name,
        quality_profile_id
    )
    .fetch_one(&mut *conn)
    .await
}

/// The open Followed Show of the Show `tmdb_id`.
async fn open_followed_show(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    // Open as `one_open_followed_show` has it.
    sqlx::query_scalar!(
        "SELECT id FROM followed_shows WHERE tmdb_id = ? AND state != 'declined'",
        tmdb_id
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Moves the Pending Followed Show `id`, and each Pending Season Request it opened, to `state`
/// with `reason`, first changing its Show's Quality to `new_quality` if given. Returns the
/// Followed Show and those Requests as they are now.
async fn decide_followed(
    conn: &mut SqliteConnection,
    id: i64,
    state: RequestState,
    reason: Option<String>,
    new_quality: Option<NewQuality>,
) -> Result<(FollowedShow, Vec<Request>), Error> {
    // Undone with the rest if the Followed Show can't be decided.
    change_followed_quality(conn, id, new_quality).await?;
    let stored_state = stored(state);
    let decided = sqlx::query!(
        "UPDATE followed_shows SET state = ?, decline_reason = ? WHERE id = ? AND state = 'pending'",
        stored_state,
        reason,
        id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        > 0;
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(Error::NoSuchFollowedShow(id))?;
    if !decided {
        return Err(Error::FollowedShowNotPending(id));
    }
    let pending = sqlx::query_scalar!(
        "SELECT id FROM requests WHERE followed_show_id = ? AND state = 'pending'",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut requests = Vec::with_capacity(pending.len());
    for request in pending {
        requests.push(decide(conn, request, state, reason.clone()).await?);
    }
    Ok((followed, requests))
}

/// `request` as its Fulfiller is handed it: naming its Followed Show only while that is Approved,
/// which asks the Fulfiller to follow the Show.
// ponytail: only a hand-off asks to follow, so a Followed Show that hands nothing off, such as one
// whose aired Seasons are all in a Library or already have open Requests, isn't followed until a
// Fulfiller contract call asks to follow without a hand-off.
async fn as_handed_off(
    conn: &mut SqliteConnection,
    mut request: Request,
) -> Result<Request, sqlx::Error> {
    if let Some(id) = request.followed_show_id
        && !followed_show_approved(conn, id).await?
    {
        request.followed_show_id = None;
    }
    Ok(request)
}

async fn followed_show_approved(conn: &mut SqliteConnection, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM followed_shows WHERE id = ? AND state = 'approved')
           AS "is!: bool""#,
        id
    )
    .fetch_one(&mut *conn)
    .await
}

/// Removes the Followed Show `id` as [`remove_followed`] does. Returns the event that says so,
/// and what [`remove_followed`] returns.
async fn cancel_followed(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<(Event, Option<Request>), Error> {
    let followed = load_followed(conn, Some(id), None).await?.pop();
    let followed = followed.ok_or(Error::NoSuchFollowedShow(id))?;
    let followed_season = remove_followed(conn, id).await?;
    let cancelled = event(FollowedShowCancelled {
        followed_show: followed.into(),
        ..Default::default()
    });
    Ok((cancelled, followed_season))
}

/// Removes the Followed Show `id`, and its Requesters with it. Returns the first Season Request it
/// opened, as it was, if it was Approved, so its Fulfiller was asked to follow the Show.
///
/// Doesn't check that `id` exists: callers load the Followed Show first, for its event.
async fn remove_followed(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Option<Request>, sqlx::Error> {
    let followed_season = if followed_show_approved(conn, id).await? {
        // ponytail: a Followed Show stores no Show ids, so once every Season Request it opened is
        // cancelled its Fulfiller isn't told to stop; store them if that bites.
        let first = sqlx::query_scalar!(
            "SELECT MIN(id) FROM requests WHERE followed_show_id = ?",
            id
        )
        .fetch_one(&mut *conn)
        .await?;
        match first {
            Some(first) => load_one(conn, first).await?,
            None => None,
        }
    } else {
        None
    };
    sqlx::query!("DELETE FROM followed_shows WHERE id = ?", id)
        .execute(&mut *conn)
        .await?;
    Ok(followed_season)
}

/// Takes `user` off the Followed Show `id`, among those `visible` lets them see, removing it as
/// [`remove_followed`] does if they were its last Requester. Returns the event for what changed,
/// and what [`remove_followed`] returns.
async fn withdraw_followed(
    conn: &mut SqliteConnection,
    user: i64,
    visible: Option<i64>,
    id: i64,
) -> Result<(Event, Option<Request>), Error> {
    let followed = load_followed(conn, Some(id), visible).await?.pop();
    let followed = followed.ok_or(Error::NoSuchFollowedShow(id))?;
    if !followed.requesters.iter().any(|r| r.user_id == user) {
        return Err(Error::NotAFollowedShowRequester(id));
    }
    if followed.requesters.len() == 1 {
        let followed_season = remove_followed(conn, id).await?;
        let withdrawn = event(FollowedShowWithdrawn {
            followed_show: followed.into(),
            ..Default::default()
        });
        return Ok((withdrawn, followed_season));
    }
    sqlx::query!(
        "DELETE FROM followed_show_requesters WHERE followed_show_id = ? AND user_id = ?",
        id,
        user
    )
    .execute(&mut *conn)
    .await?;
    let removed = event(FollowedShowRequesterRemoved {
        followed_show: followed.into(),
        user_id: user,
        ..Default::default()
    });
    Ok((removed, None))
}

/// The Followed Shows, newest first: only the one with the ID `id` if given, and only those the
/// User `requester` is a Requester of if given.
async fn load_followed(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    requester: Option<i64>,
) -> Result<Vec<FollowedShow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", tmdb_id, title, state, created_at, decline_reason, quality_id,
                  quality_name, quality_profile_id
           FROM followed_shows
           WHERE (?1 IS NULL OR id = ?1)
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM followed_show_requesters
                                        WHERE followed_show_id = followed_shows.id
                                          AND user_id = ?2))
           ORDER BY id DESC"#,
        id,
        requester
    )
    .fetch_all(&mut *conn)
    .await?;
    // ponytail: every Followed Show's Requesters, as `load` reads every Request's.
    let mut requesters: HashMap<i64, Vec<Requester>> = HashMap::new();
    let requester_rows = sqlx::query!(
        "SELECT r.followed_show_id, r.user_id, users.display_username, r.requested_at
         FROM followed_show_requesters AS r
         JOIN users ON users.id = r.user_id
         WHERE ?1 IS NULL OR r.followed_show_id = ?1
         ORDER BY r.requested_at, r.rowid",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    for row in requester_rows {
        requesters
            .entry(row.followed_show_id)
            .or_default()
            .push(Requester {
                user_id: row.user_id,
                name: row.display_username,
                requested_at: row.requested_at,
                ..Default::default()
            });
    }
    Ok(rows
        .into_iter()
        .map(|row| FollowedShow {
            id: row.id,
            show_tmdb_id: row.tmdb_id,
            show_title: row.title,
            state: read_state(&row.state).into(),
            requesters: requesters.remove(&row.id).unwrap_or_default(),
            created_at: row.created_at,
            decline_reason: row.decline_reason,
            quality: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
                .into(),
            ..Default::default()
        })
        .collect())
}

async fn load_one(conn: &mut SqliteConnection, id: i64) -> Result<Option<Request>, sqlx::Error> {
    Ok(load(conn, Some(id), None).await?.into_iter().next())
}

/// The Requests, newest first: only the one with the ID `id` if given, and only those the User
/// `requester` is a Requester of if given.
async fn load(
    conn: &mut SqliteConnection,
    id: Option<i64>,
    requester: Option<i64>,
) -> Result<Vec<Request>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", tmdb_id, season AS "season: u32", title, release_date, show_imdb_id,
                  show_tvdb_id, state, created_at, decline_reason, failure_reason, followed_show_id,
                  quality_id, quality_name, quality_profile_id, quality_changed_from,
                  quality_changed_at, quality_used_id, quality_used_name,
                  quality_used_profile_id,
                  (SELECT display_username FROM users WHERE id = quality_changed_by)
                      AS quality_changed_by
           FROM requests
           WHERE (?1 IS NULL OR id = ?1)
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM requesters
                                        WHERE request_id = requests.id AND user_id = ?2))
           ORDER BY id DESC"#,
        id,
        requester
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut requesters = requesters_by_request(conn, id).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let mut request = Request {
                id: row.id,
                state: read_state(&row.state).into(),
                requesters: requesters.remove(&row.id).unwrap_or_default(),
                created_at: row.created_at,
                decline_reason: row.decline_reason,
                failure_reason: row.failure_reason,
                followed_show_id: row.followed_show_id,
                quality: stored_quality(row.quality_id, row.quality_name, row.quality_profile_id)
                    .into(),
                quality_change: row
                    .quality_changed_at
                    .map(|changed_at| QualityChange {
                        changed_by: row.quality_changed_by,
                        previous_name: row.quality_changed_from,
                        changed_at,
                        ..Default::default()
                    })
                    .into(),
                quality_used: stored_quality(
                    row.quality_used_id,
                    row.quality_used_name,
                    row.quality_used_profile_id,
                )
                .into(),
                ..Default::default()
            };
            match row.season {
                Some(season_number) => {
                    request.season = RequestedSeason {
                        show_tmdb_id: row.tmdb_id,
                        season_number,
                        show_title: row.title,
                        air_date: row.release_date,
                        show_imdb_id: row.show_imdb_id,
                        show_tvdb_id: row.show_tvdb_id,
                        ..Default::default()
                    }
                    .into();
                }
                None => {
                    request.movie = RequestedMovie {
                        tmdb_id: row.tmdb_id,
                        title: row.title,
                        release_date: row.release_date,
                        ..Default::default()
                    }
                    .into();
                }
            }
            request
        })
        .collect())
}

/// The Requesters of the Request `id`, or of every Request if `None`, by Request id, first asker
/// first.
async fn requesters_by_request(
    conn: &mut SqliteConnection,
    id: Option<i64>,
) -> Result<HashMap<i64, Vec<Requester>>, sqlx::Error> {
    // ponytail: every Request's Requesters, even for one User's Requests; filter by them in SQL if
    // the list gets big.
    let mut requesters: HashMap<i64, Vec<Requester>> = HashMap::new();
    let requester_rows = sqlx::query!(
        "SELECT requesters.request_id, requesters.user_id, users.display_username,
                requesters.requested_at
         FROM requesters
         JOIN users ON users.id = requesters.user_id
         WHERE ?1 IS NULL OR requesters.request_id = ?1
         ORDER BY requesters.requested_at, requesters.rowid",
        id
    )
    .fetch_all(&mut *conn)
    .await?;
    for row in requester_rows {
        requesters
            .entry(row.request_id)
            .or_default()
            .push(Requester {
                user_id: row.user_id,
                name: row.display_username,
                requested_at: row.requested_at,
                ..Default::default()
            });
    }
    Ok(requesters)
}

/// The three columns a Request stores `quality` in, all set or all `NULL`, as [`stored_quality`]
/// reads them back.
fn quality_columns(
    quality: Option<&RequestedQuality>,
) -> (Option<i64>, Option<&str>, Option<&str>) {
    (
        quality.map(|quality| quality.id),
        quality.map(|quality| quality.name.as_str()),
        quality.map(|quality| quality.profile_id.as_str()),
    )
}

/// A Quality a Request stored, if it has one, from the columns [`quality_columns`] gives.
fn stored_quality(
    id: Option<i64>,
    name: Option<String>,
    profile_id: Option<String>,
) -> Option<RequestedQuality> {
    Some(RequestedQuality {
        id: id?,
        name: name?,
        profile_id: profile_id?,
        ..Default::default()
    })
}

/// A Processing Request, as [`hand_off::HandOff::poll_progress`] asks its Fulfiller about it.
struct Processing {
    id: i64,
    /// Whether it asks for a Movie, rather than a Season.
    is_movie: bool,
    /// What its Fulfiller gave when it took it.
    reference: String,
}

async fn processing(conn: &mut SqliteConnection) -> Result<Vec<Processing>, sqlx::Error> {
    sqlx::query_as!(
        Processing,
        r#"SELECT id AS "id!", season IS NULL AS "is_movie!: bool",
                  fulfiller_reference AS "reference!"
           FROM requests
           WHERE state = 'processing' AND fulfiller_reference IS NOT NULL"#
    )
    .fetch_all(&mut *conn)
    .await
}

/// The id of the Request an ask opened or approved, if `happened` says that, which that ask hands
/// off if it is Approved.
fn handed_off_id(happened: &Event) -> Option<i64> {
    match &happened.event {
        Some(Happened::RequestCreated(created)) => Some(created.request.id),
        Some(Happened::RequestStateChanged(changed))
            if changed.request.state == RequestState::Approved =>
        {
            Some(changed.request.id)
        }
        _ => None,
    }
}

/// The event saying `request` is now in the state it has.
fn state_changed(request: Request) -> Event {
    event(RequestStateChanged {
        request: request.into(),
        ..Default::default()
    })
}

/// `state` as `requests.state` stores it. Unspecified has no stored form: `""` fails the CHECK.
fn stored(state: RequestState) -> &'static str {
    STATES
        .iter()
        .find(|&&(known, _)| known == state)
        .map_or("", |&(_, name)| name)
}

fn read_state(stored: &str) -> RequestState {
    STATES
        .iter()
        .find(|&&(_, name)| name == stored)
        .map_or(RequestState::REQUEST_STATE_UNSPECIFIED, |&(state, _)| state)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap, HashSet},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use futures_util::{Stream, StreamExt};
    use nama_plugin_api::{
        BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
        fulfiller::{Download, Fulfilled, Fulfiller, Movies},
        metadata::{Episode, ExternalId, ExternalIds, Season},
        settings::{SettingValue, Settings, SettingsSchema},
    };
    use nama_plugin_tmdb::{
        Tmdb,
        testing::{API_KEY, FakeTmdb},
    };
    use nama_proto::nama::v1::{
        Permission, Request, RequestState, SubscribeResponse, event::Event as Happened,
        subscribe_response,
    };
    use sqlx::SqliteConnection;
    use tokio::sync::Notify;

    use super::{Requests, Wanted, ask, ask_movie, availability, load_one, opening_state};
    use crate::{
        contents::{Change, Contents, Decided, Decisions, Item, Metadata},
        db::Database,
        events::{EventBus, Filter},
        libraries::{Kind, Libraries},
        matching::Matcher,
        plugins::Registry,
        scanner::Scanner,
        server_settings::ServerUrl,
        users::{Permissions, User},
    };

    const TODAY: &str = "2026-11-08";

    fn episode(number: u32, air_date: Option<&str>) -> Episode {
        Episode {
            ids: ExternalIds::default(),
            season: 1,
            number,
            title: format!("Episode {number}"),
            overview: None,
            air_date: air_date.map(str::to_owned),
            runtime_minutes: None,
            still: None,
        }
    }

    #[test]
    fn a_season_still_airing_follows_how_many_of_its_aired_episodes_are_matched() {
        let airing = [
            episode(1, Some("2026-11-01")),
            episode(2, Some(TODAY)),
            episode(3, Some("2026-11-15")),
            episode(4, None),
        ];

        let all_aired = availability(&airing, &HashSet::from([1, 2]), TODAY);
        let some_aired = availability(&airing, &HashSet::from([1, 3, 4]), TODAY);
        let none_aired = availability(&airing, &HashSet::from([3, 4]), TODAY);

        assert_eq!(all_aired, Some(RequestState::Available));
        assert_eq!(some_aired, Some(RequestState::PartiallyAvailable));
        assert_eq!(none_aired, None);
    }

    #[test]
    fn a_season_yet_to_air_is_not_in_a_library() {
        let announced = [episode(1, Some("2027-01-01")), episode(2, None)];

        assert_eq!(
            availability(&announced, &HashSet::from([1, 2]), TODAY),
            None
        );
        assert_eq!(availability(&[], &HashSet::new(), TODAY), None);
    }

    /// Asks for each of `wanted` in turn as a new User, opening Requests in `state`, and returns
    /// the Requests' ids.
    async fn ask_each(
        conn: &mut SqliteConnection,
        wanted: Vec<Wanted>,
        state: RequestState,
    ) -> Result<Vec<i64>, sqlx::Error> {
        let user = insert_ada(conn).await?;
        let mut ids = Vec::new();
        for one in wanted {
            let (request, _) = ask(conn, user, one, state).await?;
            ids.push(request.id);
        }
        Ok(ids)
    }

    /// Adds the User [`ada`] stands for, and returns the id.
    async fn insert_ada(conn: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "INSERT INTO users (username, display_username, permissions, created_at, updated_at)
             VALUES ('ada', 'Ada', 0, 0, 0) RETURNING id",
        )
        .fetch_one(&mut *conn)
        .await
    }

    #[tokio::test]
    async fn a_movie_and_a_show_with_the_same_tmdb_id_have_requests_of_their_own() {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(data.path()).await.unwrap();
        let wanted = [None, Some(0), Some(1), None].map(|season| Wanted {
            tmdb_id: 1,
            season,
            title: "Shared".to_owned(),
            release_date: None,
            show_ids: ExternalIds::default(),
            followed_show: None,
            quality: None,
            quality_chosen: false,
        });

        let ids = db
            .write(move |tx| Box::pin(ask_each(tx, wanted.into(), RequestState::Pending)))
            .await
            .unwrap();

        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 3, "{ids:?}");
        assert_eq!(
            ids.first(),
            ids.last(),
            "asking again joins the Movie's Request"
        );
    }

    /// A Movie Fulfiller that counts the Requests it is handed, and takes each only once `open`
    /// lets it.
    #[derive(Default)]
    struct Gated {
        handed: AtomicUsize,
        open: Notify,
    }

    impl Fulfiller<Movies> for Gated {
        fn fulfill<'a>(&'a self, _: &'a Request) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            self.handed.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                self.open.notified().await;
                Ok(Fulfilled {
                    reference: "taken".to_owned(),
                    ..Fulfilled::default()
                })
            })
        }

        fn progress<'a>(
            &'a self,
            _: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }
    }

    /// A Movie Fulfiller that takes every Request at once, giving its TMDB id as the reference,
    /// and keeps each batch of references it is asked about.
    #[derive(Default)]
    struct Asked(std::sync::Mutex<Vec<Vec<String>>>);

    impl Fulfiller<Movies> for Asked {
        fn fulfill<'a>(
            &'a self,
            request: &'a Request,
        ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
            let reference = request.movie.tmdb_id.to_string();
            Box::pin(async {
                Ok(Fulfilled {
                    reference,
                    ..Fulfilled::default()
                })
            })
        }

        fn progress<'a>(
            &'a self,
            references: &'a [String],
        ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
            self.0.lock().unwrap().push(references.to_vec());
            Box::pin(async { Ok(HashMap::new()) })
        }
    }

    impl Asked {
        fn batches(&self) -> Vec<Vec<String>> {
            self.0.lock().unwrap().clone()
        }

        /// The references it was last asked about, sorted.
        fn last_asked_about(&self) -> Vec<String> {
            let mut batch = self.0.lock().unwrap().last().cloned().unwrap_or_default();
            batch.sort();
            batch
        }
    }

    /// A Plugin, `gated`, providing a Movie Fulfiller.
    struct FulfillerPlugin(Arc<dyn Fulfiller<Movies>>);

    impl Plugin for FulfillerPlugin {
        fn manifest(&self) -> Manifest {
            Manifest {
                id: PluginId::from_static("gated"),
                name: String::new(),
                description: String::new(),
                version: "0.0.0".to_owned(),
                settings: SettingsSchema::default(),
                extension_points: vec![PointKey::provide::<dyn Fulfiller<Movies>>()],
                logo_svg: None,
            }
        }

        fn register(&self, registrar: &mut Registrar) {
            registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::clone(&self.0));
        }
    }

    /// The ids of a title with the TMDB id `tmdb_id`.
    fn tmdb(tmdb_id: i64) -> ExternalIds {
        ExternalIds {
            tmdb: Some(tmdb_id),
            ..ExternalIds::default()
        }
    }

    /// A Match to the title with `ids`, or for an Episode to its Show's `show_ids`.
    fn matched(ids: ExternalIds, show_ids: ExternalIds) -> Decided {
        Decided::Matched(Box::new(Metadata {
            plugin: PluginId::from_static("tmdb"),
            ids,
            show_ids,
            title: "Matched".to_owned(),
            original_title: None,
            show_title: None,
            overview: None,
            release_date: None,
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        }))
    }

    /// Season 1, as the metadata source gives it: Episode 1 aired in 2017, and Episode 2 airs on
    /// `episode_2_airs`.
    fn season_1(episode_2_airs: &str) -> Season {
        Season {
            ids: ExternalIds::default(),
            number: 1,
            title: "Season 1".to_owned(),
            overview: None,
            air_date: None,
            poster: None,
            episodes: vec![
                episode(1, Some("2017-12-01")),
                episode(2, Some(episode_2_airs)),
            ],
        }
    }

    /// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
    /// and whose one Plugin provides the returned [`Gated`] as the Movie Fulfiller.
    async fn approved_for_gated(tmdb_ids: &[i64]) -> (tempfile::TempDir, Requests, Arc<Gated>) {
        let gated = Arc::new(Gated::default());
        let fulfiller = Arc::clone(&gated);
        let (data, requests) = approved_for(tmdb_ids, fulfiller).await;
        (data, requests, gated)
    }

    /// The Movie with the TMDB id `tmdb_id`, as the metadata source gives it.
    fn movie(tmdb_id: i64) -> Wanted {
        Wanted {
            tmdb_id,
            season: None,
            title: format!("Movie {tmdb_id}"),
            release_date: None,
            show_ids: ExternalIds::default(),
            followed_show: None,
            quality: None,
            quality_chosen: false,
        }
    }

    /// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
    /// and whose one Plugin provides `fulfiller` as the Movie Fulfiller.
    async fn approved_for(
        tmdb_ids: &[i64],
        fulfiller: Arc<dyn Fulfiller<Movies>>,
    ) -> (tempfile::TempDir, Requests) {
        approved_on(Registry::new(), tmdb_ids, fulfiller).await
    }

    /// [`approved_for`], with `plugins` enabled too.
    async fn approved_on(
        plugins: Registry,
        tmdb_ids: &[i64],
        fulfiller: Arc<dyn Fulfiller<Movies>>,
    ) -> (tempfile::TempDir, Requests) {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        db.write(|tx| Box::pin(insert_ada(tx))).await.unwrap();
        plugins
            .enable(Arc::new(FulfillerPlugin(fulfiller)))
            .unwrap();
        let events = EventBus::new(Arc::clone(&db), plugins.clone(), ServerUrl::default());
        let matcher = Matcher::new(Arc::clone(&db), plugins.clone(), events.clone());
        let requests = Requests::new(db, matcher, events, plugins);
        for &tmdb_id in tmdb_ids {
            ask_as(&requests, Permissions::ADMIN, tmdb_id).await;
        }
        (data, requests)
    }

    /// Waits until `asked` has been asked about `references`, sorted, in one batch.
    async fn until_asked_about(asked: &Asked, references: &[&str]) {
        let wait = async {
            while asked.last_asked_about() != references {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .expect("never asked about them in one batch");
    }

    /// How many times `asked` is asked about anything while `quiet` passes.
    async fn asked_while(asked: &Asked, quiet: Duration) -> usize {
        let before = asked.batches().len();
        tokio::time::sleep(quiet).await;
        asked.batches().len() - before
    }

    /// [`ada`] holding `permissions` asks for the Movie `tmdb_id`, as [`Requests::create`] does
    /// once the metadata source has answered. Returns the Request's id.
    async fn ask_as(requests: &Requests, permissions: Permissions, tmdb_id: i64) -> i64 {
        let asker = ada(permissions);
        let state = opening_state(&asker, Permission::AutoApproveMovie);
        let (request, _) = requests
            .db
            .write(move |tx| Box::pin(ask_movie(tx, asker.id, movie(tmdb_id), state)))
            .await
            .unwrap();
        request.id
    }

    /// A new Library Matches a file to each of the Movies `tmdb_ids`, as a scan does, which makes
    /// their open Requests Available.
    async fn match_in_a_library(requests: &Requests, tmdb_ids: &[i64]) {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().canonicalize().unwrap();
        let library = Libraries::new(Arc::clone(&requests.db))
            .create(
                "Movies",
                Kind::Movies,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        let stores = tmdb_ids
            .iter()
            .map(|&tmdb_id| Change::Store {
                path: format!("/media/{tmdb_id}.mkv"),
                size: 1,
                modified_ns: 0,
                items: vec![Item::Movie {
                    title: format!("Movie {tmdb_id}"),
                    year: None,
                    hint: Some(ExternalId::Tmdb(tmdb_id)),
                }],
            })
            .collect();
        requests.contents.apply(library, stores).await.unwrap();
        let files = requests.contents.files(library).await.unwrap();
        let decided: Vec<_> = files
            .into_iter()
            .flat_map(|file| file.items)
            .map(|stored| {
                let Item::Movie {
                    hint: Some(ExternalId::Tmdb(tmdb_id)),
                    ..
                } = stored.item
                else {
                    panic!("not a Movie stored above: {stored:?}");
                };
                (stored.id, matched(tmdb(tmdb_id), ExternalIds::default()))
            })
            .collect();
        requests
            .contents
            .record(Decisions::from(decided))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn one_poll_asks_the_fulfiller_once_about_every_processing_request() {
        let asked = Arc::new(Asked::default());
        let fulfiller = Arc::clone(&asked);
        let (_data, requests) = approved_for(&[949, 438_631, 603], fulfiller).await;
        come_up(&requests, "gated").await;
        match_in_a_library(&requests, &[603]).await;

        let any = requests.hand_off.poll_progress().await.unwrap();

        assert!(any);
        assert_eq!(asked.batches().len(), 1);
        assert_eq!(asked.last_asked_about(), ["438631", "949"]);
    }

    #[tokio::test]
    async fn the_fulfiller_is_asked_about_all_its_processing_requests_at_once_only_while_any_is() {
        const EVERY: Duration = Duration::from_millis(10);
        let asked = Arc::new(Asked::default());
        let fulfiller = Arc::clone(&asked);
        let (_data, requests) = approved_for(&[949, 438_631], fulfiller).await;
        let pending = ask_as(&requests, Permissions::NEW_USER, 603).await;
        let following = requests.clone();
        let follow = tokio::spawn(async move { following.follow_progress(EVERY).await });
        let nothing_processing = asked_while(&asked, EVERY * 10).await;

        come_up(&requests, "gated").await;
        until_asked_about(&asked, &["438631", "949"]).await;
        match_in_a_library(&requests, &[949, 438_631]).await;
        // Asked once more at most, if it was already asking as they became Available.
        asked_while(&asked, EVERY * 5).await;
        let nothing_processing_again = asked_while(&asked, EVERY * 10).await;
        requests
            .approve(&ada(Permissions::ADMIN), pending, None)
            .await
            .unwrap();

        until_asked_about(&asked, &["603"]).await;
        follow.abort();
        assert_eq!(nothing_processing, 0);
        assert_eq!(nothing_processing_again, 0);
    }

    #[tokio::test]
    async fn a_request_being_handed_off_is_not_handed_to_the_fulfiller_again() {
        let (_data, requests, gated) = approved_for_gated(&[949]).await;
        let handing_first = requests.clone();
        let first = tokio::spawn(async move { come_up(&handing_first, "gated").await });
        while gated.handed.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        tokio::time::timeout(Duration::from_secs(1), come_up(&requests, "gated"))
            .await
            .expect("the second hand-off waited on the Fulfiller");
        gated.open.notify_one();
        first.await.unwrap();

        assert_eq!(gated.handed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_plugin_coming_up_without_a_fulfiller_hands_nothing_off() {
        let (_data, requests, gated) = approved_for_gated(&[949]).await;

        come_up(&requests, "tmdb").await;

        assert_eq!(gated.handed.load(Ordering::SeqCst), 0);
    }

    /// Tells `requests` the Plugin `plugin` came up, and waits until it has handed off what that
    /// asks for.
    async fn come_up(requests: &Requests, plugin: &'static str) {
        let (came_up, queue) = tokio::sync::mpsc::unbounded_channel();
        came_up.send(PluginId::from_static(plugin)).unwrap();
        drop(came_up);
        requests.hand_off_as_fulfillers_come_up(queue).await;
    }

    /// Ada, the User the tests ask as, holding `permissions`.
    fn ada(permissions: Permissions) -> User {
        User {
            id: 1,
            name: "ada".to_owned(),
            permissions,
            created_at: 0,
        }
    }

    /// Runs `change` for a caller that stops waiting once its write is queued, as hyper drops a
    /// handler's future when its client goes away, then lets the write run.
    async fn leave_once_queued<T: Send + 'static>(
        requests: &Requests,
        change: impl Future<Output = T> + Send + 'static,
    ) {
        let busy = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let db = Arc::clone(&requests.db);
        let (writing, held) = (Arc::clone(&busy), Arc::clone(&release));
        let holding = tokio::spawn(async move {
            db.write(move |_| Box::pin(hold_writer(writing, held)))
                .await
        });
        busy.notified().await;
        let caller = tokio::spawn(change);
        let queued = async {
            while requests.db.queued_writes() == 0 {
                tokio::task::yield_now().await;
            }
        };
        tokio::time::timeout(Duration::from_secs(5), queued)
            .await
            .expect("the change never queued its write");

        caller.abort();
        assert!(caller.await.is_err_and(|ended| ended.is_cancelled()));
        release.notify_one();
        holding.await.unwrap().unwrap();
    }

    /// Enables `tmdb` as the metadata Provider of `requests`.
    fn enable_tmdb(requests: &Requests, tmdb: &FakeTmdb) {
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(API_KEY.to_owned()),
        )]));
        requests
            .hand_off
            .plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
    }

    /// A write that tells `writing` it runs, then holds the writer until `released`.
    async fn hold_writer(writing: Arc<Notify>, released: Arc<Notify>) -> Result<(), sqlx::Error> {
        writing.notify_one();
        released.notified().await;
        Ok(())
    }

    /// What the next event `published` lets through says happened, waiting a second at most.
    async fn next_happened(
        published: &mut (impl Stream<Item = SubscribeResponse> + Unpin),
    ) -> Happened {
        let response = tokio::time::timeout(Duration::from_secs(1), published.next())
            .await
            .expect("nothing was published")
            .expect("the bus closed");
        match response.message {
            Some(subscribe_response::Message::Event(event)) => event.event.unwrap(),
            other => panic!("not an event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_auto_approved_movie_asked_for_by_a_caller_who_left_is_published_and_handed_off() {
        let tmdb = FakeTmdb::start().await;
        let (_data, requests) = approved_for(&[], Arc::new(Asked::default())).await;
        enable_tmdb(&requests, &tmdb);
        let mut published = Box::pin(requests.events.subscribe(Filter::default()));
        let asking = requests.clone();

        leave_once_queued(&requests, async move {
            asking.create(&ada(Permissions::ADMIN), 348, None).await
        })
        .await;

        let created = next_happened(&mut published).await;
        assert!(
            matches!(created, Happened::RequestCreated(_)),
            "{created:?}"
        );
        let Happened::RequestStateChanged(handed_off) = next_happened(&mut published).await else {
            panic!("the Request was not handed off");
        };
        assert_eq!(handed_off.request.state, RequestState::Processing);
    }

    #[tokio::test]
    async fn a_request_declined_by_a_caller_who_left_is_published() {
        let (_data, requests) = approved_for(&[], Arc::new(Asked::default())).await;
        ask_as(&requests, Permissions::NEW_USER, 949).await;
        let mut published = Box::pin(requests.events.subscribe(Filter::default()));
        let declining = requests.clone();

        leave_once_queued(&requests, async move { declining.decline(1, "").await }).await;

        let Happened::RequestStateChanged(declined) = next_happened(&mut published).await else {
            panic!("not a change of state");
        };
        assert_eq!(declined.request.state, RequestState::Declined);
    }

    #[tokio::test]
    async fn a_request_cancelled_by_a_caller_who_left_is_published() {
        let (_data, requests) = approved_for(&[949], Arc::new(Asked::default())).await;
        let mut published = Box::pin(requests.events.subscribe(Filter::default()));
        let cancelling = requests.clone();

        leave_once_queued(&requests, async move { cancelling.cancel(1).await }).await;

        let cancelled = next_happened(&mut published).await;
        assert!(
            matches!(cancelled, Happened::RequestCancelled(_)),
            "{cancelled:?}"
        );
    }

    #[tokio::test]
    async fn a_request_withdrawn_by_a_caller_who_left_is_published() {
        let (_data, requests) = approved_for(&[], Arc::new(Asked::default())).await;
        ask_as(&requests, Permissions::NEW_USER, 949).await;
        let mut published = Box::pin(requests.events.subscribe(Filter::default()));
        let withdrawing = requests.clone();

        leave_once_queued(&requests, async move {
            withdrawing.withdraw(&ada(Permissions::NEW_USER), 1).await
        })
        .await;

        let withdrawn = next_happened(&mut published).await;
        assert!(
            matches!(withdrawn, Happened::RequestWithdrawn(_)),
            "{withdrawn:?}"
        );
    }

    #[tokio::test]
    async fn a_match_pinned_by_a_caller_who_left_publishes_it_and_the_request_it_made_available() {
        let tmdb = FakeTmdb::start().await;
        let folder = tempfile::tempdir().unwrap();
        let (_data, requests) = approved_for(&[949], Arc::new(Asked::default())).await;
        enable_tmdb(&requests, &tmdb);
        let root = folder.path().canonicalize().unwrap();
        let library = Libraries::new(Arc::clone(&requests.db))
            .create(
                "Movies",
                Kind::Movies,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        let heat = Item::Movie {
            title: "Heat".to_owned(),
            year: Some(1995),
            hint: None,
        };
        let store = Change::Store {
            path: "/media/Heat (1995)/Heat (1995).mkv".to_owned(),
            size: 1,
            modified_ns: 0,
            items: vec![heat],
        };
        requests.contents.apply(library, vec![store]).await.unwrap();
        let item = requests.contents.files(library).await.unwrap()[0].items[0].id;
        let scanner = Scanner::new(
            Arc::clone(&requests.db),
            requests.hand_off.plugins.clone(),
            requests.events.clone(),
        );
        let scan = scanner.lock(library).await;
        let mut published = Box::pin(requests.events.subscribe(Filter::default()));
        let pinning = requests.matcher.clone();

        leave_once_queued(&requests, async move {
            pinning.set_match(&scan, item, &ExternalId::Tmdb(949)).await
        })
        .await;

        let Happened::ItemMatched(matched) = next_happened(&mut published).await else {
            panic!("the Match was not published");
        };
        assert_eq!(matched.item_id, item);
        let Happened::RequestStateChanged(available) = next_happened(&mut published).await else {
            panic!("the Request was not made Available");
        };
        assert_eq!(available.request.state, RequestState::Available);
    }

    #[tokio::test]
    async fn an_available_season_request_stays_available_when_a_later_episode_airs() {
        const DARK: i64 = 70523;
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let root = folder.path().canonicalize().unwrap();
        let library = Libraries::new(Arc::clone(&db))
            .create(
                "Shows",
                Kind::Shows,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        let contents = Contents::new(Arc::clone(&db));
        let pilot = Item::Episode {
            show: "Dark".to_owned(),
            year: None,
            hint: Some(ExternalId::Tmdb(DARK)),
            season: 1,
            episode: 1,
        };
        let store = Change::Store {
            path: "/media/Dark/Season 01/Dark - S01E01.mkv".to_owned(),
            size: 1,
            modified_ns: 0,
            items: vec![pilot],
        };
        contents.apply(library, vec![store]).await.unwrap();
        let item = contents.files(library).await.unwrap()[0].items[0].id;
        let wanted = Wanted {
            tmdb_id: DARK,
            season: Some(1),
            title: "Dark".to_owned(),
            release_date: None,
            show_ids: ExternalIds::default(),
            followed_show: None,
            quality: None,
            quality_chosen: false,
        };
        let request = db
            .write(move |tx| Box::pin(ask_each(tx, vec![wanted], RequestState::Pending)))
            .await
            .unwrap()[0];
        let scan = |episode_2_airs| Decisions {
            items: vec![(item, matched(ExternalIds::default(), tmdb(DARK)))],
            seasons: vec![(DARK, season_1(episode_2_airs))],
        };
        contents.record(scan("2099-12-01")).await.unwrap();

        let episode_2_aired = contents.record(scan("2017-12-08")).await.unwrap();

        let state = db
            .write(move |tx| Box::pin(load_one(tx, request)))
            .await
            .unwrap()
            .unwrap()
            .state;
        assert_eq!(state.as_known(), Some(RequestState::Available));
        assert!(episode_2_aired.is_empty(), "{episode_2_aired:?}");
    }
}
