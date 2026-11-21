//! Helpers the unit tests of Requests share.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar, contract,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
    metadata::{ExternalId, ExternalIds},
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    Event, FollowedShow, Permission, Request, RequestState, RequestedMovie, RequestedSeason,
    SubscribeResponse, event::Event as Happened, request::MovieOrSeason, subscribe_response,
};
use sqlx::SqliteConnection;

use super::apply::Applied;
use super::asking::{Wanted, WantedSeasons, ask, ask_movie, opening_state};
use super::availability::make_seasons_available;
use super::followed::follow;
use super::state::{STATES, settle};
use super::{Requests, qualities};
use crate::{
    db::Database,
    events::EventBus,
    library::{
        Change, Contents, Decided, Decisions, Item, Libraries, LibraryKind, Metadata,
        SourceEpisode, SourceSeason,
        fixture::{MatchedMovie, movies_library, tmdb_movie},
    },
    plugins::{Answer, ExtensionPoints, Failure, Registry},
    server_settings::ServerUrl,
    users::{Permissions, User},
};

/// The Movie `request` asks for, panicking if it asks for a Season.
pub(super) fn asked_movie(request: &Request) -> &RequestedMovie {
    match &request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => movie,
        other => panic!("not a Movie Request: {other:?}"),
    }
}

/// The Season `request` asks for, panicking if it asks for a Movie.
pub(super) fn asked_season(request: &Request) -> &RequestedSeason {
    match &request.movie_or_season {
        Some(MovieOrSeason::Season(season)) => season,
        other => panic!("not a Season Request: {other:?}"),
    }
}

/// The Movie a Fulfiller is told `request` asks for, panicking if it asks for a Season.
pub(super) fn told_movie(request: &contract::Request) -> &contract::RequestedMovie {
    match &request.movie_or_season {
        Some(contract::request::MovieOrSeason::Movie(movie)) => movie,
        other => panic!("not a Movie Request: {other:?}"),
    }
}

/// The Season a Fulfiller is told `request` asks for, panicking if it asks for a Movie.
pub(super) fn told_season(request: &contract::Request) -> &contract::RequestedSeason {
    match &request.movie_or_season {
        Some(contract::request::MovieOrSeason::Season(season)) => season,
        other => panic!("not a Season Request: {other:?}"),
    }
}

pub(super) fn episode(number: u32, air_date: Option<&str>) -> SourceEpisode {
    SourceEpisode {
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

/// Asks for each of `wanted` in turn as a new User, opening Requests in `state`, and returns
/// the Requests' ids.
pub(super) async fn ask_each(
    conn: &mut SqliteConnection,
    wanted: Vec<Wanted>,
    state: RequestState,
) -> Result<Vec<i64>, sqlx::Error> {
    let user = insert_ada(conn).await?;
    let mut ids = Vec::new();
    for one in wanted {
        ids.push(ask(conn, user, one, state).await?.value.id);
    }
    Ok(ids)
}

/// Adds the User [`ada`] stands for, and returns the id.
pub(super) async fn insert_ada(conn: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO users (username, display_username, permissions, created_at, updated_at)
         VALUES ('ada', 'Ada', 0, 0, 0) RETURNING id",
    )
    .fetch_one(&mut *conn)
    .await
}

/// A Fulfiller for Movies and Shows that takes every Request at once, giving a Movie's TMDB id
/// or `season-<number>` as the reference, keeps each batch of references it is asked about,
/// and says it is downloading [`downloading`] for each. Its own, since a `ScriptedFulfiller` says
/// nothing is downloading.
#[derive(Default)]
pub(super) struct Asked(pub(super) std::sync::Mutex<Vec<Vec<String>>>);

impl Fulfiller<Movies> for Asked {
    fn fulfill<'a>(
        &'a self,
        request: &'a contract::Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        let ids = &told_movie(request).ids;
        // The contract has no key id: these tests keep a Movie under its TMDB id.
        let tmdb = ids.iter().find(|id| id.namespace == "tmdb");
        took(tmdb.expect("a TMDB id").value.clone())
    }

    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        self.answer(references)
    }
}

impl Fulfiller<Shows> for Asked {
    fn fulfill<'a>(
        &'a self,
        request: &'a contract::Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        took(format!("season-{}", told_season(request).season_number))
    }

    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        self.answer(references)
    }
}

/// A Fulfiller of Movies that takes every Request as [`Asked`] does, but never says how one is
/// progressing, notifying `.0` once it is asked. Its own, since a `ScriptedFulfiller` answers at
/// once.
#[derive(Default)]
pub(super) struct NeverAnswers(pub(super) tokio::sync::Notify);

impl Fulfiller<Movies> for NeverAnswers {
    fn fulfill<'a>(
        &'a self,
        request: &'a contract::Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        let ids = &told_movie(request).ids;
        let tmdb = ids.iter().find(|id| id.namespace == "tmdb");
        took(tmdb.expect("a TMDB id").value.clone())
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        self.0.notify_one();
        Box::pin(std::future::pending())
    }
}

/// A Fulfiller's answer that it took a Request, with `reference`.
pub(super) fn took(reference: String) -> BoxFuture<'static, Result<Fulfilled, PluginError>> {
    Box::pin(async {
        Ok(Fulfilled {
            reference,
            ..Fulfilled::default()
        })
    })
}

/// What [`Asked`] says it is downloading for `reference`.
pub(super) fn downloading(reference: &str) -> Vec<Download> {
    vec![Download {
        title: reference.to_owned(),
        ..Download::default()
    }]
}

impl Asked {
    fn answer(
        &self,
        references: &[String],
    ) -> BoxFuture<'static, Result<HashMap<String, Vec<Download>>, PluginError>> {
        self.0.lock().unwrap().push(references.to_vec());
        let answered = references
            .iter()
            .map(|reference| (reference.clone(), downloading(reference)))
            .collect();
        Box::pin(async { Ok(answered) })
    }

    pub(super) fn batches(&self) -> Vec<Vec<String>> {
        self.0.lock().unwrap().clone()
    }

    /// The references it was last asked about, sorted.
    pub(super) fn last_asked_about(&self) -> Vec<String> {
        let mut batch = self.0.lock().unwrap().last().cloned().unwrap_or_default();
        batch.sort();
        batch
    }
}

/// A Plugin with the id `.0`, providing `.1` as the Fulfiller of `Kind`, Movies or Shows.
pub(super) struct FulfillerPlugin<Kind>(
    pub(super) &'static str,
    pub(super) Arc<dyn Fulfiller<Kind>>,
);

impl<Kind: 'static> Plugin for FulfillerPlugin<Kind> {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Fulfiller<Kind>>()],
            ..Manifest::new(PluginId::from_static(self.0))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Fulfiller<Kind>>(Priority::DEFAULT, Arc::clone(&self.1));
        Ok(())
    }
}

/// Enables `tmdb` as a title source in `plugins`.
pub(super) fn enable_tmdb(plugins: &Registry, tmdb: &FakeTmdb) {
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text(API_KEY.to_owned()),
    )]));
    plugins
        .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
        .unwrap();
}

/// The ids of a title with the TMDB id `tmdb_id`.
pub(super) fn tmdb(tmdb_id: i64) -> ExternalIds {
    ExternalIds::from_iter([ExternalId::tmdb(tmdb_id)])
}

/// A Match to the title with `ids`, or for an Episode to its Show's `show_ids`.
pub(super) fn matched(ids: ExternalIds, show_ids: ExternalIds) -> Decided {
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
        titled_backdrop: None,
        logo: None,
        still: None,
        genres: Vec::new(),
        content_rating: None,
        show_poster: None,
        show_overview: None,
        show_first_air_date: None,
        score: None,
        original_language: None,
    }))
}

/// Season 1, as the title source gives it: Episode 1 aired in 2017, and Episode 2 airs on
/// `episode_2_airs`.
pub(super) fn season_1(episode_2_airs: &str) -> SourceSeason {
    SourceSeason {
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

/// The Movie with the TMDB id `tmdb_id`, as the title source gives it.
pub(super) fn movie(tmdb_id: i64) -> Wanted {
    Wanted {
        id: ExternalId::tmdb(tmdb_id),
        season: None,
        title: format!("Movie {tmdb_id}"),
        release_date: None,
        ids: ExternalIds::default(),
        followed_show: None,
        quality: qualities::Ask::default(),
    }
}

/// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
/// and whose one Plugin provides `fulfiller` as the Movie Fulfiller.
pub(super) async fn approved_for(
    tmdb_ids: &[i64],
    fulfiller: Arc<dyn Fulfiller<Movies>>,
) -> (tempfile::TempDir, Requests) {
    approved_on(&Registry::new(), tmdb_ids, fulfiller).await
}

/// [`approved_for`], with `plugins` enabled too, and `fulfiller` as the Fulfiller of `Kind`.
pub(super) async fn approved_on<Kind: 'static>(
    plugins: &Registry,
    tmdb_ids: &[i64],
    fulfiller: Arc<dyn Fulfiller<Kind>>,
) -> (tempfile::TempDir, Requests) {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    db.write(|tx| Box::pin(insert_ada(tx))).await.unwrap();
    plugins
        .enable(Arc::new(FulfillerPlugin("gated", fulfiller)))
        .unwrap();
    let requests = requests_on(db, plugins.extension_points());
    for &tmdb_id in tmdb_ids {
        ask_as(&requests, Permissions::ADMIN, tmdb_id).await;
    }
    (data, requests)
}

/// Requests in `db`, handing off to the Fulfillers in `plugins`.
pub(super) fn requests_on(db: Arc<Database>, plugins: ExtensionPoints) -> Requests {
    let events = EventBus::new(Arc::clone(&db), plugins.clone(), ServerUrl::default());
    Requests::new(db, events, plugins)
}

/// [`ada`] holding `permissions` asks for the Movie `tmdb_id`, as [`Requests::create`] does
/// once the title source has answered. Returns the Request's id.
pub(super) async fn ask_as(requests: &Requests, permissions: Permissions, tmdb_id: i64) -> i64 {
    let asker = ada(permissions);
    let state = opening_state(&asker, Permission::AutoApproveMovie);
    let applied = requests
        .db
        .write(move |tx| Box::pin(ask_movie(tx, asker.id, movie(tmdb_id), state)))
        .await
        .unwrap();
    applied.value.id
}

/// A new Library Matches a file to each of the Movies `tmdb_ids` in turn, which makes their
/// open Requests Available.
pub(super) async fn match_in_a_library(db: &Arc<Database>, tmdb_ids: &[i64]) {
    let folder = tempfile::tempdir().unwrap();
    let library = movies_library(db, folder.path()).await;
    let contents = Contents::new(Arc::clone(db));
    for &tmdb_id in tmdb_ids {
        let path = format!("/media/{tmdb_id}.mkv");
        let movie = tmdb_movie(tmdb_id, &format!("Movie {tmdb_id}"));
        MatchedMovie::store(&contents, library, &path, movie).await;
    }
}

/// A Movie Request in each state, without and with a Fulfiller's reference. Returns each one's
/// id, state and reference.
pub(super) async fn insert_in_each_state(
    conn: &mut SqliteConnection,
) -> Result<Vec<(i64, RequestState, Option<String>)>, sqlx::Error> {
    let mut inserted = Vec::new();
    for (tmdb_id, &(state, stored)) in (1..).zip(&STATES) {
        for reference in [None, Some(format!("movie-{tmdb_id}"))] {
            let id = sqlx::query_scalar(
                "INSERT INTO requests (title_id, title, state, created_at, fulfiller_reference)
                 VALUES (?, 'Movie', ?, 0, ?)
                 RETURNING id",
            )
            // A Movie of its own each, as only one open Request per Movie may be stored.
            .bind(ExternalId::tmdb(tmdb_id * 2 + i64::from(reference.is_some())).to_string())
            .bind(stored)
            .bind(&reference)
            .fetch_one(&mut *conn)
            .await?;
            inserted.push((id, state, reference));
        }
    }
    Ok(inserted)
}

/// Tells `requests` the Plugin `plugin` came up, and waits until it has handed off what that
/// asks for.
pub(super) async fn come_up(requests: &Requests, plugin: &'static str) {
    let (came_up, queue) = tokio::sync::mpsc::unbounded_channel();
    came_up.send(PluginId::from_static(plugin)).unwrap();
    drop(came_up);
    requests.as_fulfillers_come_up(queue).await;
}

/// Ada, the User the tests ask as, holding `permissions`.
pub(super) fn ada(permissions: Permissions) -> User {
    User {
        id: 1,
        name: "ada".to_owned(),
        permissions,
        created_at: 0,
    }
}

/// What the next event `published` lets through says happened, waiting a second at most.
pub(super) async fn next_happened(
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

/// What each event in `applied` says happened, as [`said`] tells it.
pub(super) fn happened<T>(applied: &Applied<T>) -> Vec<(&'static str, RequestState)> {
    said(&applied.events)
}

/// What each of `events` says happened, and the state its Request or Followed Show is in by
/// then.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "the writes under test publish only Request and Followed Show events"
)]
pub(super) fn said(events: &[Event]) -> Vec<(&'static str, RequestState)> {
    events
        .iter()
        .map(|event| {
            let (what, state) = match event.event.as_ref().expect("an event") {
                Happened::RequestCreated(created) => ("created", created.request.state),
                Happened::RequesterAdded(added) => ("joined", added.request.state),
                Happened::RequestStateChanged(changed) => ("moved", changed.request.state),
                Happened::RequestCancelled(cancelled) => ("cancelled", cancelled.request.state),
                Happened::RequestWithdrawn(withdrawn) => ("withdrawn", withdrawn.request.state),
                Happened::RequesterRemoved(removed) => ("left", removed.request.state),
                Happened::FollowedShowCreated(created) => ("followed", created.followed_show.state),
                Happened::FollowedShowRequesterAdded(added) => {
                    ("joined the Show", added.followed_show.state)
                }
                Happened::FollowedShowStateChanged(changed) => {
                    ("moved the Show", changed.followed_show.state)
                }
                Happened::FollowedShowCancelled(cancelled) => {
                    ("cancelled the Show", cancelled.followed_show.state)
                }
                Happened::FollowedShowWithdrawn(withdrawn) => {
                    ("withdrew the Show", withdrawn.followed_show.state)
                }
                Happened::FollowedShowRequesterRemoved(removed) => {
                    ("left the Show", removed.followed_show.state)
                }
                other => panic!("not a Request or Followed Show event: {other:?}"),
            };
            (what, state.as_known().unwrap_or_default())
        })
        .collect()
}

pub(super) const NOTHING: [i64; 0] = [];

/// The ids of the Requests `applied` hands off.
pub(super) fn handed_off<T>(applied: &Applied<T>) -> Vec<i64> {
    applied.hand_off.iter().map(|request| request.id).collect()
}

pub(super) const ADA: i64 = 1;

pub(super) const BOB: i64 = 2;

/// A new database holding the Users [`ADA`] and [`BOB`].
pub(super) async fn ada_and_bob() -> (tempfile::TempDir, Arc<Database>) {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    db.write(|tx| {
        Box::pin(async move {
            insert_ada(tx).await?;
            sqlx::query(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES ('bob', 'Bob', 0, 0, 0)",
            )
            .execute(&mut **tx)
            .await
        })
    })
    .await
    .unwrap();
    (data, db)
}

/// `user` asks for the Movie `tmdb_id`, opening a Request in `state` if it has none.
pub(super) async fn ask_for(
    db: &Database,
    user: i64,
    tmdb_id: i64,
    state: RequestState,
) -> Applied<Request> {
    db.write(move |tx| Box::pin(ask_movie(tx, user, movie(tmdb_id), state)))
        .await
        .unwrap()
}

/// The Fulfiller of the Plugin `fulfiller` took the Approved Request `id`, or didn't, and why.
pub(super) async fn settled(
    db: &Database,
    id: i64,
    fulfilled: Result<Fulfilled, String>,
) -> Applied<Option<Request>> {
    let fulfilled = answered("fulfiller", fulfilled);
    db.write(move |tx| Box::pin(settle(tx, id, fulfilled, None)))
        .await
        .unwrap()
}

/// The Kernel failed the Approved Request `id` for lack of an id in any of `needs`.
pub(super) async fn failed_needing(db: &Database, id: i64, needs: &[&str]) -> Request {
    let failure = Answer {
        plugin: PluginId::from_static("fulfiller"),
        result: Err(Failure {
            kind: needs
                .iter()
                .map(|&namespace| namespace.to_owned())
                .collect(),
            message: "needs an id".to_owned(),
        }),
    };
    db.write(move |tx| Box::pin(settle(tx, id, failure, None)))
        .await
        .unwrap()
        .value
        .unwrap()
}

/// The Request `id` as it is stored now.
pub(super) async fn read(db: &Database, id: i64) -> Request {
    let mut conn = db.reader().acquire().await.unwrap();
    super::listing::load_one(&mut conn, id)
        .await
        .unwrap()
        .unwrap()
}

/// What the Plugin `plugin`'s Fulfiller answered, as the Registry hands it back.
pub(super) fn answered(
    plugin: &'static str,
    fulfilled: Result<Fulfilled, String>,
) -> Answer<Fulfilled, Vec<String>> {
    Answer {
        plugin: PluginId::from_static(plugin),
        result: fulfilled.map_err(|message| Failure {
            kind: Vec::new(),
            message,
        }),
    }
}

/// What a Fulfiller says when it takes a Request.
pub(super) fn fulfilled() -> Fulfilled {
    Fulfilled {
        reference: "taken".to_owned(),
        ..Fulfilled::default()
    }
}

/// Dark's TMDB id.
pub(super) const DARK: i64 = 70523;

/// Seasons 1 and 2 of Dark, as the title source gives them.
pub(super) fn dark() -> WantedSeasons {
    WantedSeasons {
        show_id: ExternalId::tmdb(DARK),
        show_ids: ExternalIds::default(),
        show_title: "Dark".to_owned(),
        seasons: vec![
            season_1("2017-12-08"),
            SourceSeason {
                number: 2,
                ..season_1("2019-06-21")
            },
        ],
        quality: qualities::Ask::default(),
    }
}

/// Season `number` of Dark, as the title source gives it: both its Episodes have aired.
pub(super) fn dark_season(number: u32) -> SourceSeason {
    SourceSeason {
        number,
        title: format!("Season {number}"),
        ..season_1("2017-12-08")
    }
}

/// Dark with the Seasons `numbers`, each as [`dark_season`] gives it.
pub(super) fn dark_with_seasons(numbers: impl IntoIterator<Item = u32>) -> WantedSeasons {
    WantedSeasons {
        seasons: numbers.into_iter().map(dark_season).collect(),
        ..dark()
    }
}

/// A new Library Matches a file to each of `episodes` of Season `season` of Dark, as a scan
/// does, without Requests hearing of the Season yet: [`scan_dark`] tells them.
pub(super) async fn match_dark_episodes(db: &Arc<Database>, season: i64, episodes: &[i64]) {
    match_episodes(db, ExternalId::tmdb(DARK), season, episodes).await;
}

/// [`match_dark_episodes`], for the Show `show` in place of Dark.
pub(super) async fn match_episodes(
    db: &Arc<Database>,
    show: ExternalId,
    season: i64,
    episodes: &[i64],
) {
    match_show_episodes(db, ExternalIds::from_iter([show]), season, episodes).await;
}

/// [`match_episodes`], for a Show Matched with every id in `show`, hinted by the first.
pub(super) async fn match_show_episodes(
    db: &Arc<Database>,
    show: ExternalIds,
    season: i64,
    episodes: &[i64],
) {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().canonicalize().unwrap();
    let root = root.to_str().unwrap();
    let contents = Contents::new(Arc::clone(db));
    // Named after its folder, as a test may Match into more than one.
    let library = Libraries::new(Arc::clone(db))
        .create(root, LibraryKind::Shows, vec![root.to_owned()])
        .await
        .unwrap()
        .id;
    let stores = episodes
        .iter()
        .map(|&episode| Change::Store {
            path: format!("/media/Dark/Season {season}/Dark - S{season}E{episode}.mkv"),
            size: 1,
            modified_ns: 0,
            items: vec![Item::Episode {
                show: "Dark".to_owned(),
                year: None,
                hint: show.iter().next(),
                season,
                episode,
            }],
        })
        .collect();
    contents.apply(library, stores).await.unwrap();
    let decided: Vec<_> = contents
        .files(library)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|file| file.items)
        .map(|stored| (stored.id, matched(ExternalIds::default(), show.clone())))
        .collect();
    contents.record(Decisions::from(decided)).await.unwrap();
}

/// Tells Requests a scan fetched each of the Seasons `numbers` of Dark, as [`dark_season`]
/// gives them, as a Library's write does. Returns the events for what it changed.
pub(super) async fn scan_dark(db: &Database, numbers: &[u32]) -> Vec<Event> {
    scan(db, ExternalId::tmdb(DARK), numbers).await
}

/// [`scan_dark`], for the Show `show` in place of Dark.
pub(super) async fn scan(db: &Database, show: ExternalId, numbers: &[u32]) -> Vec<Event> {
    let seasons = numbers
        .iter()
        .map(|&number| (ExternalIds::from_iter([show.clone()]), dark_season(number)))
        .collect();
    db.write(move |tx| Box::pin(make_seasons_available(tx, seasons)))
        .await
        .unwrap()
}

/// `user` follows `show`, opening its Followed Show in `state` if it has none.
pub(super) async fn follow_as(
    db: &Database,
    user: i64,
    show: WantedSeasons,
    state: RequestState,
) -> Applied<(FollowedShow, Vec<Request>)> {
    db.write(move |tx| Box::pin(follow(tx, user, show, state)))
        .await
        .unwrap()
}

/// The ids of the Season Requests `followed` asked for.
pub(super) fn season_ids(followed: &Applied<(FollowedShow, Vec<Request>)>) -> Vec<i64> {
    followed.value.1.iter().map(|request| request.id).collect()
}

/// Breaking Bad, as [`dark`] gives Dark.
pub(super) fn breaking_bad() -> WantedSeasons {
    WantedSeasons {
        show_id: ExternalId::tmdb(1396),
        show_title: "Breaking Bad".to_owned(),
        ..dark()
    }
}

pub(super) fn user(id: i64, permissions: Permissions) -> User {
    User {
        id,
        ..ada(permissions)
    }
}
