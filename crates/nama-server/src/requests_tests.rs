use std::{
    collections::{BTreeMap, HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
    metadata::{Episode, ExternalId, ExternalIds, Season},
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    Event, FollowedShow, ListRequestsResponse, Permission, Request, RequestState,
    SubscribeResponse, event::Event as Happened, subscribe_response,
};
use sqlx::SqliteConnection;
use tokio::sync::Notify;

use super::{
    Applied, Error, MovieOrSeason, RequestedMovie, RequestedSeason, Requests, STATES, Wanted,
    WantedSeasons, approve, ask, ask_movie, ask_seasons, asked, availability, cancel,
    cancel_followed, decide_followed, decline, follow, is_open, load, load_one,
    make_seasons_available, opening_state, progress, qualities, retry, settle, withdraw,
    withdraw_followed,
};
use crate::{
    artwork::Artwork,
    custom_images::CustomImages,
    db::Database,
    events::{EventBus, Filter},
    images::ImageStore,
    libraries::{Kind, Libraries},
    library::contents::{
        Change, Contents, Decided, Decisions, Item, Metadata,
        fixture::{MatchedMovie, movies_library, tmdb_movie},
    },
    plugins::{Answer, ExtensionPoints, Failure, Registry},
    scanner::Scanner,
    server_settings::ServerUrl,
    users::{Permissions, User},
};

const TODAY: &str = "2026-11-08";

/// The Movie `request` asks for, panicking if it asks for a Season.
fn asked_movie(request: &Request) -> &RequestedMovie {
    match &request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => movie,
        other => panic!("not a Movie Request: {other:?}"),
    }
}

/// The Season `request` asks for, panicking if it asks for a Movie.
fn asked_season(request: &Request) -> &RequestedSeason {
    match &request.movie_or_season {
        Some(MovieOrSeason::Season(season)) => season,
        other => panic!("not a Season Request: {other:?}"),
    }
}

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
        ids.push(ask(conn, user, one, state).await?.value.id);
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
        quality: qualities::Ask::default(),
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

/// The interval a test polls progress at: how long a poll waits for the fake Fulfillers, which
/// answer at once.
const INTERVAL: Duration = Duration::from_secs(5);

/// A Fulfiller for Movies and Shows that keeps the ids of the Requests it is handed, in order,
/// and takes each only once `open` lets it.
#[derive(Default)]
struct Gated {
    handed: std::sync::Mutex<Vec<i64>>,
    open: Notify,
}

impl Gated {
    fn handed(&self) -> Vec<i64> {
        self.handed.lock().unwrap().clone()
    }

    fn take<'a>(&'a self, request: &Request) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        self.handed.lock().unwrap().push(request.id);
        Box::pin(async {
            self.open.notified().await;
            Ok(Fulfilled {
                reference: "taken".to_owned(),
                ..Fulfilled::default()
            })
        })
    }
}

impl Fulfiller<Movies> for Gated {
    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        self.take(request)
    }

    fn progress<'a>(
        &'a self,
        _: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

impl Fulfiller<Shows> for Gated {
    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        self.take(request)
    }

    fn progress<'a>(
        &'a self,
        _: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// A Fulfiller for Movies and Shows that takes every Request at once, giving a Movie's TMDB id
/// or `season-<number>` as the reference, keeps each batch of references it is asked about,
/// and says it is downloading [`downloading`] for each.
#[derive(Default)]
struct Asked(std::sync::Mutex<Vec<Vec<String>>>);

impl Fulfiller<Movies> for Asked {
    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        took(asked_movie(request).tmdb_id.to_string())
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
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        took(format!("season-{}", asked_season(request).season_number))
    }

    fn progress<'a>(
        &'a self,
        references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        self.answer(references)
    }
}

/// A Fulfiller's answer that it took a Request, with `reference`.
fn took(reference: String) -> BoxFuture<'static, Result<Fulfilled, PluginError>> {
    Box::pin(async {
        Ok(Fulfilled {
            reference,
            ..Fulfilled::default()
        })
    })
}

/// What [`Asked`] says it is downloading for `reference`.
fn downloading(reference: &str) -> Vec<Download> {
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

/// A Plugin with the id `.0`, providing `.1` as the Fulfiller of `Kind`, Movies or Shows.
struct FulfillerPlugin<Kind>(&'static str, Arc<dyn Fulfiller<Kind>>);

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
        still: None,
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
        quality: qualities::Ask::default(),
    }
}

/// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
/// and whose one Plugin provides `fulfiller` as the Movie Fulfiller.
async fn approved_for(
    tmdb_ids: &[i64],
    fulfiller: Arc<dyn Fulfiller<Movies>>,
) -> (tempfile::TempDir, Requests) {
    approved_on(&Registry::new(), tmdb_ids, fulfiller).await
}

/// [`approved_for`], with `plugins` enabled too, and `fulfiller` as the Fulfiller of `Kind`.
async fn approved_on<Kind: 'static>(
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
fn requests_on(db: Arc<Database>, plugins: ExtensionPoints) -> Requests {
    let events = EventBus::new(Arc::clone(&db), plugins.clone(), ServerUrl::default());
    Requests::new(db, events, plugins)
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
    let applied = requests
        .db
        .write(move |tx| Box::pin(ask_movie(tx, asker.id, movie(tmdb_id), state)))
        .await
        .unwrap();
    applied.value.id
}

/// A new Library Matches a file to each of the Movies `tmdb_ids` in turn, which makes their
/// open Requests Available.
async fn match_in_a_library(db: &Arc<Database>, tmdb_ids: &[i64]) {
    let folder = tempfile::tempdir().unwrap();
    let library = movies_library(db, folder.path()).await;
    let contents = Contents::new(Arc::clone(db));
    for &tmdb_id in tmdb_ids {
        let path = format!("/media/{tmdb_id}.mkv");
        let movie = tmdb_movie(tmdb_id, &format!("Movie {tmdb_id}"));
        MatchedMovie::store(&contents, library, &path, movie).await;
    }
}

#[tokio::test]
async fn one_poll_asks_the_fulfiller_once_about_every_processing_request() {
    let asked = Arc::new(Asked::default());
    let fulfiller = Arc::clone(&asked);
    let (_data, requests) = approved_for(&[949, 438_631, 603], fulfiller).await;
    come_up(&requests, "gated").await;
    match_in_a_library(&requests.db, &[603]).await;

    let any = requests.progress.poll(INTERVAL).await.unwrap();

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
    let follow = tokio::spawn(async move { following.progress.follow(EVERY).await });
    let nothing_processing = asked_while(&asked, EVERY * 10).await;

    come_up(&requests, "gated").await;
    until_asked_about(&asked, &["438631", "949"]).await;
    match_in_a_library(&requests.db, &[949, 438_631]).await;
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
async fn only_a_request_its_fulfiller_has_that_isnt_available_yet_is_asked_about_and_gets_downloads()
 {
    let asked = Arc::new(Asked::default());
    let fulfiller = Arc::clone(&asked);
    let (_data, requests) = approved_for(&[], fulfiller).await;
    let inserted = requests
        .db
        .write(|tx| Box::pin(insert_in_each_state(tx)))
        .await
        .unwrap();

    requests.progress.poll(INTERVAL).await.unwrap();

    let admin = ada(Permissions::ADMIN);
    let mut has_it = Vec::new();
    for (id, state, reference) in inserted {
        let not_available_yet = matches!(
            state,
            RequestState::Processing | RequestState::PartiallyAvailable
        );
        let expected = reference.is_some() && not_available_yet;
        let request = requests.get(&admin, id).await.unwrap();
        assert_eq!(
            !request.downloads.is_empty(),
            expected,
            "{state:?} with the reference {reference:?}"
        );
        has_it.extend(reference.filter(|_| expected));
    }
    has_it.sort();
    assert_eq!(asked.last_asked_about(), has_it);
}

/// A Movie Request in each state, without and with a Fulfiller's reference. Returns each one's
/// id, state and reference.
async fn insert_in_each_state(
    conn: &mut SqliteConnection,
) -> Result<Vec<(i64, RequestState, Option<String>)>, sqlx::Error> {
    let mut inserted = Vec::new();
    for (tmdb_id, &(state, stored)) in (1..).zip(&STATES) {
        for reference in [None, Some(format!("movie-{tmdb_id}"))] {
            let id = sqlx::query_scalar(
                "INSERT INTO requests (tmdb_id, title, state, created_at, fulfiller_reference)
                 VALUES (?, 'Movie', ?, 0, ?)
                 RETURNING id",
            )
            // A Movie of its own each, as only one open Request per Movie may be stored.
            .bind(tmdb_id * 2 + i64::from(reference.is_some()))
            .bind(stored)
            .bind(&reference)
            .fetch_one(&mut *conn)
            .await?;
            inserted.push((id, state, reference));
        }
    }
    Ok(inserted)
}

#[tokio::test]
async fn sql_and_rust_agree_on_which_requests_are_open() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let inserted = db
        .write(|tx| Box::pin(insert_in_each_state(tx)))
        .await
        .unwrap();
    macro_rules! read {
        ($open:literal) => {
            sqlx::query_scalar!(r#"SELECT id AS "id!" FROM requests WHERE "# + $open)
        };
    }
    let open: HashSet<i64> = open_request_sql!(read)
        .fetch_all(db.reader())
        .await
        .unwrap()
        .into_iter()
        .collect();

    for (id, state, reference) in inserted {
        let expected = is_open(state);
        assert_eq!(open.contains(&id), expected, "{state:?} with {reference:?}");
    }
}

#[tokio::test]
async fn sql_and_rust_agree_on_which_requests_their_fulfiller_has() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    let inserted = db
        .write(|tx| Box::pin(insert_in_each_state(tx)))
        .await
        .unwrap();
    macro_rules! read {
        ($fulfilling:literal) => {
            sqlx::query_scalar!(r#"SELECT id AS "id!" FROM requests WHERE "# + $fulfilling)
        };
    }
    let fulfilling: HashSet<i64> = progress::fulfilling_sql!(read)
        .fetch_all(db.reader())
        .await
        .unwrap()
        .into_iter()
        .collect();

    for (id, state, reference) in inserted {
        let request = Request {
            state: state.into(),
            ..Default::default()
        };
        let expected = progress::is_fulfilling(&request);
        assert_eq!(
            fulfilling.contains(&id),
            expected,
            "{state:?} with {reference:?}"
        );
    }
}

#[tokio::test]
async fn only_a_declined_followed_show_is_closed() {
    let data = tempfile::tempdir().unwrap();
    let db = Database::open(data.path()).await.unwrap();
    db.write(|tx| {
        Box::pin(
            sqlx::query(
                "INSERT INTO followed_shows (id, tmdb_id, title, state, created_at)
                 VALUES (1, 1, 'Show', 'pending', 0), (2, 2, 'Show', 'approved', 0),
                        (3, 3, 'Show', 'declined', 0)",
            )
            .execute(&mut **tx),
        )
    })
    .await
    .unwrap();
    macro_rules! read {
        ($open:literal) => {
            sqlx::query_scalar!(r#"SELECT id AS "id!" FROM followed_shows WHERE "# + $open)
        };
    }
    let open: HashSet<i64> = open_followed_show_sql!(read)
        .fetch_all(db.reader())
        .await
        .unwrap()
        .into_iter()
        .collect();

    assert_eq!(open, HashSet::from([1, 2]));
}

#[tokio::test]
async fn a_handed_off_season_gets_progress_while_partially_available_but_not_once_available() {
    let asked = Arc::new(Asked::default());
    let (_data, db) = ada_and_bob().await;
    let plugins = Registry::new();
    let fulfiller: Arc<dyn Fulfiller<Shows>> = Arc::<Asked>::clone(&asked);
    plugins
        .enable(Arc::new(FulfillerPlugin("sonarr", fulfiller)))
        .unwrap();
    let plugins = plugins.extension_points();
    let bus = EventBus::new(Arc::clone(&db), plugins.clone(), ServerUrl::default());
    let requests = Requests::new(Arc::clone(&db), bus.clone(), plugins);
    let season_1 = WantedSeasons {
        seasons: vec![dark_season(1)],
        ..dark()
    };
    let id = db
        .write(|tx| Box::pin(ask_seasons(tx, ADA, season_1, RequestState::Approved)))
        .await
        .unwrap()
        .value[0]
        .id;
    come_up(&requests, "sonarr").await;
    match_dark_episodes(&db, 1, &[1]).await;
    scan_dark(&db, &[1]).await;
    let mut events = Box::pin(bus.subscribe_as_admin(Filter::default()));

    let any = requests.progress.poll(INTERVAL).await.unwrap();
    let Happened::RequestProgress(progress) = next_happened(&mut events).await else {
        panic!("its progress was not published");
    };
    let partly = requests.get(&ada(Permissions::ADMIN), id).await.unwrap();
    match_dark_episodes(&db, 1, &[2]).await;
    scan_dark(&db, &[1]).await;
    let batches = asked.batches().len();
    let any_once_available = requests.progress.poll(INTERVAL).await.unwrap();
    // `poll` publishes before it returns, so anything it published is waiting here.
    let published_once_available =
        tokio::time::timeout(Duration::from_millis(100), events.next()).await;
    let available = requests.get(&ada(Permissions::ADMIN), id).await.unwrap();

    assert!(any);
    assert_eq!(asked.last_asked_about(), ["season-1"]);
    let published = progress.request.into_option().unwrap();
    assert_eq!(
        (published.id, published.state),
        (id, RequestState::PartiallyAvailable.into())
    );
    assert_eq!(published.downloads, downloading("season-1"));
    assert_eq!(partly.downloads, downloading("season-1"));
    assert!(!any_once_available);
    assert_eq!(asked.batches().len(), batches);
    assert_eq!(published_once_available.ok(), None);
    assert_eq!(available.state, RequestState::Available);
    assert_eq!(available.downloads, []);
}

#[tokio::test]
async fn a_request_being_handed_off_is_not_handed_to_the_fulfiller_again() {
    let (_data, requests, gated) = approved_for_gated(&[949]).await;
    let handing_first = requests.clone();
    let first = tokio::spawn(async move { come_up(&handing_first, "gated").await });
    while gated.handed().is_empty() {
        tokio::task::yield_now().await;
    }

    tokio::time::timeout(Duration::from_secs(1), come_up(&requests, "gated"))
        .await
        .expect("the second hand-off waited on the Fulfiller");
    gated.open.notify_one();
    first.await.unwrap();

    assert_eq!(gated.handed().len(), 1);
}

#[tokio::test]
async fn a_fulfiller_coming_up_leaves_the_requests_an_ask_is_handing_off_to_it() {
    let gated = Arc::new(Gated::default());
    let fulfiller: Arc<dyn Fulfiller<Shows>> = Arc::<Gated>::clone(&gated);
    let (_data, requests) = approved_on(&Registry::new(), &[], fulfiller).await;
    let asking = requests.clone();
    let ask = tokio::spawn(async move {
        asking
            .apply(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Approved)))
            .await
    });
    while gated.handed().is_empty() {
        tokio::task::yield_now().await;
    }

    tokio::time::timeout(Duration::from_secs(1), come_up(&requests, "gated"))
        .await
        .expect("the came-up hand-off waited on the Fulfiller");
    assert_eq!(
        gated.handed().len(),
        1,
        "the came-up hand-off took the second Request"
    );
    gated.open.notify_one();
    while gated.handed().len() < 2 {
        tokio::task::yield_now().await;
    }
    gated.open.notify_one();
    let asked = ask.await.unwrap().unwrap();

    let ids: Vec<i64> = asked.iter().map(|request| request.id).collect();
    assert_eq!(gated.handed(), ids);
    assert!(
        asked
            .iter()
            .all(|request| request.state == RequestState::Processing),
        "{asked:?}"
    );
}

#[tokio::test]
async fn a_plugin_that_provides_no_fulfiller_coming_up_hands_nothing_off() {
    let (_data, requests, gated) = approved_for_gated(&[949]).await;

    come_up(&requests, "tmdb").await;

    assert_eq!(gated.handed(), NOTHING);
}

/// Tells `requests` the Plugin `plugin` came up, and waits until it has handed off what that
/// asks for.
async fn come_up(requests: &Requests, plugin: &'static str) {
    let (came_up, queue) = tokio::sync::mpsc::unbounded_channel();
    came_up.send(PluginId::from_static(plugin)).unwrap();
    drop(came_up);
    requests.as_fulfillers_come_up(queue).await;
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

/// Enables `tmdb` as the metadata Provider in `plugins`.
fn enable_tmdb(plugins: &Registry, tmdb: &FakeTmdb) {
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text(API_KEY.to_owned()),
    )]));
    plugins
        .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
        .unwrap();
}

/// A write that tells `writing` it runs, then holds the writer until `released`.
async fn hold_writer(writing: Arc<Notify>, released: Arc<Notify>) -> Result<(), sqlx::Error> {
    writing.notify_one();
    released.notified().await;
    Ok(())
}

/// Every event `requests` publishes from now on.
fn subscribe(requests: &Requests) -> Pin<Box<impl Stream<Item = SubscribeResponse> + use<>>> {
    Box::pin(requests.events.subscribe_as_admin(Filter::default()))
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
    let plugins = Registry::new();
    enable_tmdb(&plugins, &tmdb);
    let (_data, requests) = approved_on::<Movies>(&plugins, &[], Arc::new(Asked::default())).await;
    let mut published = subscribe(&requests);
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
    let mut published = subscribe(&requests);
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
    let mut published = subscribe(&requests);
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
    let mut published = subscribe(&requests);
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
    let plugins = Registry::new();
    enable_tmdb(&plugins, &tmdb);
    let (data, requests) =
        approved_on::<Movies>(&plugins, &[949], Arc::new(Asked::default())).await;
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
    let images = ImageStore::new(data.path(), Arc::clone(&requests.db));
    let scanner = Scanner::new(
        Arc::clone(&requests.db),
        requests.plugins.clone(),
        Artwork::new(images.clone(), requests.plugins.clone()),
        CustomImages::new(Arc::clone(&requests.db), images),
        requests.events.clone(),
        None,
        None,
        None,
    );
    let mut published = subscribe(&requests);
    let pinning = scanner.matcher().clone();

    leave_once_queued(&requests, async move {
        pinning.set_match(item, &ExternalId::Tmdb(949)).await
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
        quality: qualities::Ask::default(),
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

/// What each event in `applied` says happened, as [`said`] tells it.
fn happened<T>(applied: &Applied<T>) -> Vec<(&'static str, RequestState)> {
    said(&applied.events)
}

/// What each of `events` says happened, and the state its Request or Followed Show is in by
/// then.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "the writes under test publish only Request and Followed Show events"
)]
fn said(events: &[Event]) -> Vec<(&'static str, RequestState)> {
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

const NOTHING: [i64; 0] = [];

/// The ids of the Requests `applied` hands off.
fn handed_off<T>(applied: &Applied<T>) -> Vec<i64> {
    applied.hand_off.iter().map(|request| request.id).collect()
}

/// The id of the Season Request whose Show `applied` stops following, if any.
fn unfollowed<T>(applied: &Applied<T>) -> Option<i64> {
    applied.unfollow.as_ref().map(|season| season.id)
}

const ADA: i64 = 1;
const BOB: i64 = 2;

/// A new database holding the Users [`ADA`] and [`BOB`].
async fn ada_and_bob() -> (tempfile::TempDir, Arc<Database>) {
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
async fn ask_for(db: &Database, user: i64, tmdb_id: i64, state: RequestState) -> Applied<Request> {
    db.write(move |tx| Box::pin(ask_movie(tx, user, movie(tmdb_id), state)))
        .await
        .unwrap()
}

/// The Fulfiller of the Plugin `fulfiller` took the Approved Request `id`, or didn't, and why.
async fn settled(
    db: &Database,
    id: i64,
    fulfilled: Result<Fulfilled, String>,
) -> Applied<Option<Request>> {
    let fulfilled = answered("fulfiller", fulfilled);
    db.write(move |tx| Box::pin(settle(tx, id, fulfilled, None)))
        .await
        .unwrap()
}

/// What the Plugin `plugin`'s Fulfiller answered, as the Registry hands it back.
fn answered(plugin: &'static str, fulfilled: Result<Fulfilled, String>) -> Answer<Fulfilled> {
    Answer {
        plugin: PluginId::from_static(plugin),
        result: fulfilled.map_err(|message| Failure { kind: (), message }),
    }
}

/// What a Fulfiller says when it takes a Request.
fn fulfilled() -> Fulfilled {
    Fulfilled {
        reference: "taken".to_owned(),
        ..Fulfilled::default()
    }
}

#[tokio::test]
async fn asking_for_a_movie_opens_its_request_and_hands_it_off_only_if_approved() {
    let (_data, db) = ada_and_bob().await;

    let pending = ask_for(&db, ADA, 949, RequestState::Pending).await;
    let approved = ask_for(&db, ADA, 603, RequestState::Approved).await;

    assert_eq!(happened(&pending), [("created", RequestState::Pending)]);
    assert_eq!(handed_off(&pending), NOTHING);
    assert_eq!(happened(&approved), [("created", RequestState::Approved)]);
    assert_eq!(handed_off(&approved), [approved.value.id]);
}

#[tokio::test]
async fn joining_a_pending_request_approves_and_hands_it_off_only_for_an_auto_approver() {
    let (_data, db) = ada_and_bob().await;
    ask_for(&db, ADA, 949, RequestState::Pending).await;

    let joined = ask_for(&db, BOB, 949, RequestState::Pending).await;
    let asked_again = ask_for(&db, BOB, 949, RequestState::Approved).await;

    assert_eq!(happened(&joined), [("joined", RequestState::Pending)]);
    assert_eq!(handed_off(&joined), NOTHING);
    assert_eq!(happened(&asked_again), [("moved", RequestState::Approved)]);
    assert_eq!(handed_off(&asked_again), [joined.value.id]);
}

#[tokio::test]
async fn joining_a_request_already_approved_hands_nothing_off() {
    let (_data, db) = ada_and_bob().await;
    ask_for(&db, ADA, 949, RequestState::Approved).await;
    let processing = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, processing, Ok(fulfilled())).await;
    let failed = ask_for(&db, ADA, 348, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, failed, Err("no space".to_owned())).await;

    let mut joined = Vec::new();
    for tmdb_id in [949, 603, 348] {
        let applied = ask_for(&db, BOB, tmdb_id, RequestState::Approved).await;
        assert_eq!(handed_off(&applied), NOTHING);
        joined.extend(happened(&applied));
    }

    assert_eq!(
        joined,
        [
            ("joined", RequestState::Approved),
            ("joined", RequestState::Processing),
            ("joined", RequestState::Failed)
        ]
    );
}

#[tokio::test]
async fn asking_for_a_declined_movie_opens_a_new_request_but_one_in_a_library_is_refused() {
    let (_data, db) = ada_and_bob().await;
    let declined = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;
    db.write(move |tx| Box::pin(decline(tx, declined, None)))
        .await
        .unwrap();
    ask_for(&db, ADA, 603, RequestState::Pending).await;
    match_in_a_library(&db, &[603]).await;

    let asked_again = ask_for(&db, BOB, 949, RequestState::Pending).await;
    let in_a_library = db
        .write(|tx| Box::pin(ask_movie(tx, BOB, movie(603), RequestState::Pending)))
        .await;

    assert_eq!(happened(&asked_again), [("created", RequestState::Pending)]);
    assert_ne!(asked_again.value.id, declined);
    assert!(
        matches!(in_a_library, Err(Error::InLibrary(603))),
        "{in_a_library:?}"
    );
}

/// Dark's TMDB id.
const DARK: i64 = 70523;

/// Seasons 1 and 2 of Dark, as the metadata source gives them.
fn dark() -> WantedSeasons {
    WantedSeasons {
        show_tmdb_id: DARK,
        show_ids: ExternalIds::default(),
        show_title: "Dark".to_owned(),
        seasons: vec![
            season_1("2017-12-08"),
            Season {
                number: 2,
                ..season_1("2019-06-21")
            },
        ],
        quality: qualities::Ask::default(),
    }
}

/// Season `number` of Dark, as the metadata source gives it: both its Episodes have aired.
fn dark_season(number: u32) -> Season {
    Season {
        number,
        title: format!("Season {number}"),
        ..season_1("2017-12-08")
    }
}

/// A new Library Matches a file to each of `episodes` of Season `season` of Dark, as a scan
/// does, without Requests hearing of the Season yet: [`scan_dark`] tells them.
async fn match_dark_episodes(db: &Arc<Database>, season: i64, episodes: &[i64]) {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().canonicalize().unwrap();
    let contents = Contents::new(Arc::clone(db));
    let library = Libraries::new(Arc::clone(db))
        .create(
            "Shows",
            Kind::Shows,
            vec![root.to_str().unwrap().to_owned()],
        )
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
                hint: Some(ExternalId::Tmdb(DARK)),
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
        .map(|stored| (stored.id, matched(ExternalIds::default(), tmdb(DARK))))
        .collect();
    contents.record(Decisions::from(decided)).await.unwrap();
}

/// Tells Requests a scan fetched each of the Seasons `numbers` of Dark, as [`dark_season`]
/// gives them, as a Library's write does. Returns the events for what it changed.
async fn scan_dark(db: &Database, numbers: &[u32]) -> Vec<Event> {
    let seasons = numbers
        .iter()
        .map(|&number| (DARK, dark_season(number)))
        .collect();
    db.write(move |tx| Box::pin(make_seasons_available(tx, seasons)))
        .await
        .unwrap()
}

#[tokio::test]
async fn asking_for_seasons_opens_a_request_for_each_and_hands_off_the_approved_ones() {
    let (_data, db) = ada_and_bob().await;
    let pending = db
        .write(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Pending)))
        .await
        .unwrap();
    let first = pending.value[0].id;
    db.write(move |tx| Box::pin(cancel(tx, first)))
        .await
        .unwrap();

    let approved = db
        .write(|tx| Box::pin(ask_seasons(tx, BOB, dark(), RequestState::Approved)))
        .await
        .unwrap();

    assert_eq!(
        happened(&pending),
        [
            ("created", RequestState::Pending),
            ("created", RequestState::Pending)
        ]
    );
    assert_eq!(handed_off(&pending), NOTHING);
    assert_eq!(
        happened(&approved),
        [
            ("created", RequestState::Approved),
            ("joined", RequestState::Pending),
            ("moved", RequestState::Approved)
        ]
    );
    let ids: Vec<i64> = approved.value.iter().map(|request| request.id).collect();
    assert_eq!(handed_off(&approved), ids);
}

#[tokio::test]
async fn approving_a_pending_request_hands_it_off() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;

    let approved = db
        .write(move |tx| Box::pin(approve(tx, id, None)))
        .await
        .unwrap();
    let again = db.write(move |tx| Box::pin(approve(tx, id, None))).await;

    assert_eq!(happened(&approved), [("moved", RequestState::Approved)]);
    assert_eq!(handed_off(&approved), [id]);
    assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
}

#[tokio::test]
async fn declining_a_pending_request_hands_nothing_off() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;

    let declined = db
        .write(move |tx| Box::pin(decline(tx, id, Some("No.".to_owned()))))
        .await
        .unwrap();
    let again = db.write(move |tx| Box::pin(decline(tx, id, None))).await;

    assert_eq!(happened(&declined), [("moved", RequestState::Declined)]);
    assert_eq!(handed_off(&declined), NOTHING);
    assert_eq!(declined.value.decline_reason.as_deref(), Some("No."));
    assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
}

#[tokio::test]
async fn settling_an_approved_request_moves_it_to_processing_or_failed_once() {
    let (_data, db) = ada_and_bob().await;
    let taken = ask_for(&db, ADA, 949, RequestState::Approved)
        .await
        .value
        .id;
    let refused = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;

    let processing = settled(&db, taken, Ok(fulfilled())).await;
    let failed = settled(&db, refused, Err("no space".to_owned())).await;
    let again = settled(&db, taken, Ok(fulfilled())).await;

    assert_eq!(happened(&processing), [("moved", RequestState::Processing)]);
    assert_eq!(happened(&failed), [("moved", RequestState::Failed)]);
    assert_eq!(handed_off(&processing), NOTHING);
    assert_eq!(handed_off(&failed), NOTHING);
    assert!(again.value.is_none());
    assert_eq!(again.events.len(), 0);
}

#[tokio::test]
async fn settling_a_request_names_the_plugin_whose_fulfiller_failed_it() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Approved)
        .await
        .value
        .id;
    let failed_by_radarr = answered("radarr", Err("no space".to_owned()));

    let failed = db
        .write(move |tx| Box::pin(settle(tx, id, failed_by_radarr, None)))
        .await
        .unwrap()
        .value
        .unwrap();

    assert_eq!(failed.fulfiller_plugin_id.as_deref(), Some("radarr"));
}

#[tokio::test]
async fn retrying_a_failed_request_approves_and_hands_it_off() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 949, RequestState::Approved)
        .await
        .value
        .id;
    let pending = ask_for(&db, ADA, 603, RequestState::Pending).await.value.id;
    settled(&db, id, Err("no space".to_owned())).await;

    let retried = db
        .write(move |tx| Box::pin(retry(tx, id, None)))
        .await
        .unwrap();
    let not_failed = db.write(move |tx| Box::pin(retry(tx, pending, None))).await;

    assert_eq!(happened(&retried), [("moved", RequestState::Approved)]);
    assert_eq!(handed_off(&retried), [id]);
    assert_eq!(retried.value.failure_reason, None);
    assert!(
        matches!(not_failed, Err(Error::NotFailed(_))),
        "{not_failed:?}"
    );
}

#[tokio::test]
async fn cancelling_a_request_says_so_whatever_its_state() {
    let (_data, db) = ada_and_bob().await;
    let pending = ask_for(&db, ADA, 949, RequestState::Pending).await.value.id;
    let processing = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, processing, Ok(fulfilled())).await;
    let declined = ask_for(&db, ADA, 348, RequestState::Pending).await.value.id;
    db.write(move |tx| Box::pin(decline(tx, declined, None)))
        .await
        .unwrap();
    let approved = ask_for(&db, ADA, 680, RequestState::Approved)
        .await
        .value
        .id;
    let failed = ask_for(&db, ADA, 155, RequestState::Approved)
        .await
        .value
        .id;
    settled(&db, failed, Err("no space".to_owned())).await;
    let available = ask_for(&db, ADA, 157, RequestState::Pending).await.value.id;
    match_in_a_library(&db, &[157]).await;
    let partly = db
        .write(|tx| Box::pin(ask_seasons(tx, ADA, dark(), RequestState::Approved)))
        .await
        .unwrap()
        .value[0]
        .id;
    settled(&db, partly, Ok(fulfilled())).await;
    match_dark_episodes(&db, 1, &[1]).await;
    scan_dark(&db, &[1]).await;

    let mut cancelled = Vec::new();
    for id in [
        pending, processing, declined, approved, failed, available, partly,
    ] {
        let applied = db.write(move |tx| Box::pin(cancel(tx, id))).await.unwrap();
        assert_eq!(handed_off(&applied), NOTHING);
        cancelled.extend(happened(&applied));
    }

    assert_eq!(
        cancelled,
        [
            ("cancelled", RequestState::Pending),
            ("cancelled", RequestState::Processing),
            ("cancelled", RequestState::Declined),
            ("cancelled", RequestState::Approved),
            ("cancelled", RequestState::Failed),
            ("cancelled", RequestState::Available),
            ("cancelled", RequestState::PartiallyAvailable)
        ]
    );
}

#[tokio::test]
async fn withdrawing_from_a_request_that_is_not_pending_is_refused() {
    let (_data, db) = ada_and_bob().await;
    let id = ask_for(&db, ADA, 603, RequestState::Approved)
        .await
        .value
        .id;

    let refused = db
        .write(move |tx| Box::pin(withdraw(tx, ADA, None, id)))
        .await;

    assert!(
        matches!(refused, Err(Error::NotPending(asked::Asked::Request, refused_id)) if refused_id == id),
        "{refused:?}"
    );
}

/// `user` follows `show`, opening its Followed Show in `state` if it has none.
async fn follow_as(
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
fn season_ids(followed: &Applied<(FollowedShow, Vec<Request>)>) -> Vec<i64> {
    followed.value.1.iter().map(|request| request.id).collect()
}

/// Breaking Bad, as [`dark`] gives Dark.
fn breaking_bad() -> WantedSeasons {
    WantedSeasons {
        show_tmdb_id: 1396,
        show_title: "Breaking Bad".to_owned(),
        ..dark()
    }
}

#[tokio::test]
async fn following_a_show_asks_for_each_season_and_hands_them_off_only_if_approved() {
    let (_data, db) = ada_and_bob().await;

    let pending = follow_as(&db, ADA, dark(), RequestState::Pending).await;
    let joined = follow_as(&db, BOB, dark(), RequestState::Pending).await;
    let approved = follow_as(&db, ADA, breaking_bad(), RequestState::Approved).await;

    assert_eq!(
        happened(&pending),
        [
            ("followed", RequestState::Pending),
            ("created", RequestState::Pending),
            ("created", RequestState::Pending)
        ]
    );
    assert_eq!(handed_off(&pending), NOTHING);
    assert_eq!(
        happened(&joined),
        [
            ("joined the Show", RequestState::Pending),
            ("joined", RequestState::Pending),
            ("joined", RequestState::Pending)
        ]
    );
    assert_eq!(handed_off(&joined), NOTHING);
    assert_eq!(
        happened(&approved),
        [
            ("followed", RequestState::Approved),
            ("created", RequestState::Approved),
            ("created", RequestState::Approved)
        ]
    );
    assert_eq!(handed_off(&approved), season_ids(&approved));
    for followed in [pending, joined, approved] {
        assert_eq!(unfollowed(&followed), None);
    }
}

#[tokio::test]
async fn approving_a_pending_followed_show_approves_and_hands_off_its_pending_seasons() {
    let (_data, db) = ada_and_bob().await;
    let followed = follow_as(&db, ADA, dark(), RequestState::Pending).await;
    let id = followed.value.0.id;

    let approved = db
        .write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
        .await
        .unwrap();
    let again = db
        .write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
        .await;

    assert_eq!(
        happened(&approved),
        [
            ("moved the Show", RequestState::Approved),
            ("moved", RequestState::Approved),
            ("moved", RequestState::Approved)
        ]
    );
    assert_eq!(handed_off(&approved), season_ids(&followed));
    assert_eq!(unfollowed(&approved), None);
    assert!(matches!(again, Err(Error::NotPending(..))), "{again:?}");
}

#[tokio::test]
async fn declining_a_pending_followed_show_declines_its_pending_seasons_and_hands_nothing_off() {
    let (_data, db) = ada_and_bob().await;
    let id = follow_as(&db, ADA, dark(), RequestState::Pending)
        .await
        .value
        .0
        .id;

    let declined = db
        .write(move |tx| {
            let reason = Some("No.".to_owned());
            Box::pin(decide_followed(
                tx,
                id,
                RequestState::Declined,
                reason,
                None,
            ))
        })
        .await
        .unwrap();

    assert_eq!(
        happened(&declined),
        [
            ("moved the Show", RequestState::Declined),
            ("moved", RequestState::Declined),
            ("moved", RequestState::Declined)
        ]
    );
    assert_eq!(handed_off(&declined), NOTHING);
    assert_eq!(unfollowed(&declined), None);
    assert_eq!(declined.value.decline_reason.as_deref(), Some("No."));
}

#[tokio::test]
async fn cancelling_a_followed_show_unfollows_it_only_if_it_was_approved() {
    let (_data, db) = ada_and_bob().await;
    let approved = follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let pending = follow_as(&db, ADA, breaking_bad(), RequestState::Pending).await;
    let (approved_id, pending_id) = (approved.value.0.id, pending.value.0.id);

    let cancelled_approved = db
        .write(move |tx| Box::pin(cancel_followed(tx, approved_id)))
        .await
        .unwrap();
    let cancelled_pending = db
        .write(move |tx| Box::pin(cancel_followed(tx, pending_id)))
        .await
        .unwrap();
    let again = db
        .write(move |tx| Box::pin(cancel_followed(tx, approved_id)))
        .await;

    assert_eq!(
        happened(&cancelled_approved),
        [("cancelled the Show", RequestState::Approved)]
    );
    assert_eq!(
        unfollowed(&cancelled_approved),
        season_ids(&approved).first().copied()
    );
    assert_eq!(
        happened(&cancelled_pending),
        [("cancelled the Show", RequestState::Pending)]
    );
    assert_eq!(unfollowed(&cancelled_pending), None);
    assert_eq!(handed_off(&cancelled_approved), NOTHING);
    assert_eq!(handed_off(&cancelled_pending), NOTHING);
    assert!(matches!(again, Err(Error::NoSuch(..))), "{again:?}");
}

#[tokio::test]
async fn withdrawing_from_a_followed_show_that_is_not_pending_is_allowed() {
    let (_data, db) = ada_and_bob().await;
    let id = follow_as(&db, ADA, dark(), RequestState::Approved)
        .await
        .value
        .0
        .id;
    follow_as(&db, BOB, dark(), RequestState::Pending).await;

    let bob_left = db
        .write(move |tx| Box::pin(withdraw_followed(tx, BOB, None, id)))
        .await
        .unwrap();

    assert_eq!(
        happened(&bob_left),
        [("left the Show", RequestState::Approved)]
    );
    assert_eq!(unfollowed(&bob_left), None);
}

#[tokio::test]
async fn the_last_requester_withdrawing_from_an_approved_followed_show_unfollows_it() {
    let (_data, db) = ada_and_bob().await;
    let followed = follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let id = followed.value.0.id;

    let ada_left = db
        .write(move |tx| Box::pin(withdraw_followed(tx, ADA, None, id)))
        .await
        .unwrap();

    assert_eq!(
        happened(&ada_left),
        [("withdrew the Show", RequestState::Approved)]
    );
    assert_eq!(
        unfollowed(&ada_left),
        season_ids(&followed).first().copied()
    );
}

const KINDS: [asked::Asked; 2] = [asked::Asked::Request, asked::Asked::FollowedShow];

fn user(id: i64, permissions: Permissions) -> User {
    User {
        id,
        ..ada(permissions)
    }
}

/// `user` asks for the Movie 949 or follows Dark, as `kind` says, joining it if it is open.
/// Returns what that applied, with the id of the one asked for as its value.
async fn ask_kind(db: &Database, kind: asked::Asked, user: i64) -> Applied<i64> {
    match kind {
        asked::Asked::Request => {
            let Applied {
                value,
                events,
                hand_off,
                unfollow,
            } = ask_for(db, user, 949, RequestState::Pending).await;
            Applied {
                value: value.id,
                events,
                hand_off,
                unfollow,
            }
        }
        asked::Asked::FollowedShow => {
            let Applied {
                value: (followed, _),
                events,
                hand_off,
                unfollow,
            } = follow_as(db, user, dark(), RequestState::Pending).await;
            Applied {
                value: followed.id,
                events,
                hand_off,
                unfollow,
            }
        }
    }
}

/// The ids of the Requesters `page` lists for the `kind` with the ID `id`, if it lists it.
fn listed_requesters(page: &ListRequestsResponse, kind: asked::Asked, id: i64) -> Option<Vec<i64>> {
    let requesters = match kind {
        asked::Asked::Request => page
            .requests
            .iter()
            .find(|request| request.id == id)
            .map(|request| &request.requesters),
        asked::Asked::FollowedShow => page
            .followed_shows
            .iter()
            .find(|followed| followed.id == id)
            .map(|followed| &followed.requesters),
    }?;
    Some(
        requesters
            .iter()
            .map(|requester| requester.user_id)
            .collect(),
    )
}

#[tokio::test]
async fn requesters_are_listed_in_the_order_they_asked() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let id = ask_kind(&db, kind, BOB).await.value;
        ask_kind(&db, kind, ADA).await;
        let page = requests
            .list(&ada(Permissions::ADMIN), None, 100, "")
            .await
            .unwrap();

        assert_eq!(
            listed_requesters(&page, kind, id),
            Some(vec![BOB, ADA]),
            "{kind:?}"
        );
    }
}

#[tokio::test]
async fn asking_for_an_open_one_joins_it_and_says_so() {
    let (_data, db) = ada_and_bob().await;

    for kind in KINDS {
        let opened = ask_kind(&db, kind, ADA).await.value;
        let joined = ask_kind(&db, kind, BOB).await;
        let says = match kind {
            asked::Asked::Request => "joined",
            asked::Asked::FollowedShow => "joined the Show",
        };

        assert_eq!(joined.value, opened, "{kind:?}");
        assert_eq!(
            said(&joined.events).first(),
            Some(&(says, RequestState::Pending)),
            "{kind:?}"
        );
    }
}

/// `requester` withdraws from the `kind` with the ID `id`.
async fn withdraw_kind(
    requests: &Requests,
    kind: asked::Asked,
    requester: &User,
    id: i64,
) -> Result<(), Error> {
    match kind {
        asked::Asked::Request => requests.withdraw(requester, id).await,
        asked::Asked::FollowedShow => requests.withdraw_followed(requester, id).await,
    }
}

#[tokio::test]
async fn joining_one_twice_changes_nothing() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let opened = ask_kind(&db, kind, ADA).await.value;
        let again = ask_kind(&db, kind, ADA).await;
        let page = requests
            .list(&ada(Permissions::ADMIN), None, 100, "")
            .await
            .unwrap();

        assert_eq!(again.value, opened, "{kind:?}");
        assert_eq!(said(&again.events), [], "{kind:?}");
        assert_eq!(handed_off(&again), NOTHING, "{kind:?}");
        assert_eq!(
            listed_requesters(&page, kind, again.value),
            Some(vec![ADA]),
            "{kind:?}"
        );
    }
}

#[tokio::test]
async fn a_requester_leaving_takes_only_them_off() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    let bob = user(BOB, Permissions::from_iter([Permission::ViewRequests]));

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        ask_kind(&db, kind, BOB).await;
        withdraw_kind(&requests, kind, &bob, id).await.unwrap();
        let again = withdraw_kind(&requests, kind, &bob, id).await;
        let page = requests
            .list(&ada(Permissions::ADMIN), None, 100, "")
            .await
            .unwrap();

        assert_eq!(
            listed_requesters(&page, kind, id),
            Some(vec![ADA]),
            "{kind:?}"
        );
        assert!(
            matches!(again, Err(Error::NotARequester(k, i)) if k == kind && i == id),
            "{again:?}"
        );
    }
}

#[tokio::test]
async fn the_last_requester_leaving_removes_it() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        withdraw_kind(&requests, kind, &ada(Permissions::NEW_USER), id)
            .await
            .unwrap();
        let page = requests
            .list(&ada(Permissions::ADMIN), None, 100, "")
            .await
            .unwrap();

        assert_eq!(listed_requesters(&page, kind, id), None, "{kind:?}");
    }
}

#[tokio::test]
async fn a_non_requester_withdrawing_is_not_found_unless_they_may_see_it_even_once_approved() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    let viewer = user(BOB, Permissions::from_iter([Permission::ViewRequests]));
    let non_viewer = user(BOB, Permissions::from_iter([]));

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        approve_kind(&requests, kind, id).await.unwrap();
        let unseen = withdraw_kind(&requests, kind, &non_viewer, id).await;
        let seen = withdraw_kind(&requests, kind, &viewer, id).await;

        assert!(
            matches!(unseen, Err(Error::NoSuch(k, i)) if k == kind && i == id),
            "{unseen:?}"
        );
        assert!(
            matches!(seen, Err(Error::NotARequester(k, i)) if k == kind && i == id),
            "{seen:?}"
        );
    }
}

/// A Request manager approves the `kind` with the ID `id`.
async fn approve_kind(requests: &Requests, kind: asked::Asked, id: i64) -> Result<(), Error> {
    let manager = ada(Permissions::ADMIN);
    match kind {
        asked::Asked::Request => requests.approve(&manager, id, None).await.map(drop),
        asked::Asked::FollowedShow => requests
            .approve_followed(&manager, id, None)
            .await
            .map(drop),
    }
}

/// Declines the `kind` with the ID `id`, giving `reason`. Returns the reason it keeps.
async fn decline_kind(
    requests: &Requests,
    kind: asked::Asked,
    id: i64,
    reason: &str,
) -> Result<Option<String>, Error> {
    match kind {
        asked::Asked::Request => requests
            .decline(id, reason)
            .await
            .map(|request| request.decline_reason),
        asked::Asked::FollowedShow => requests
            .decline_followed(id, reason)
            .await
            .map(|followed| followed.decline_reason),
    }
}

#[tokio::test]
async fn only_a_pending_one_is_decided_and_declining_keeps_its_reason() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());

    for kind in KINDS {
        let declined = ask_kind(&db, kind, ADA).await.value;
        let reason = decline_kind(&requests, kind, declined, " No. ")
            .await
            .unwrap();
        let approved = ask_kind(&db, kind, ADA).await.value;
        approve_kind(&requests, kind, approved).await.unwrap();
        let approved_again = approve_kind(&requests, kind, declined).await;
        let declined_again = decline_kind(&requests, kind, approved, "").await;

        assert_eq!(reason.as_deref(), Some("No."), "{kind:?}");
        assert!(
            matches!(approved_again, Err(Error::NotPending(k, id)) if k == kind && id == declined),
            "{approved_again:?}"
        );
        assert!(
            matches!(declined_again, Err(Error::NotPending(k, id)) if k == kind && id == approved),
            "{declined_again:?}"
        );
    }
}

/// Whether `viewer` sees the `kind` with the ID `id` among the Requests listed.
async fn sees(requests: &Requests, viewer: &User, kind: asked::Asked, id: i64) -> bool {
    let page = requests.list(viewer, None, 100, "").await.unwrap();
    listed_requesters(&page, kind, id).is_some()
}

#[tokio::test]
async fn only_its_requesters_and_those_who_may_view_requests_see_one() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    let nothing = Permissions::from_iter([]);
    let view_requests = Permissions::from_iter([Permission::ViewRequests]);

    for kind in KINDS {
        let id = ask_kind(&db, kind, ADA).await.value;
        let seen = [
            sees(&requests, &ada(nothing), kind, id).await,
            sees(&requests, &user(BOB, nothing), kind, id).await,
            sees(&requests, &user(BOB, view_requests), kind, id).await,
        ];

        assert_eq!(seen, [true, false, true], "{kind:?}");
    }
}

#[tokio::test]
async fn the_pending_count_is_as_many_as_the_pending_list_shows() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    ask_for(&db, ADA, 949, RequestState::Pending).await;
    ask_for(&db, ADA, 603, RequestState::Approved).await;
    follow_as(&db, ADA, dark(), RequestState::Pending).await;
    follow_as(&db, ADA, breaking_bad(), RequestState::Approved).await;
    let nothing = Permissions::from_iter([]);
    let viewers = [
        user(BOB, Permissions::from_iter([Permission::ViewRequests])),
        ada(nothing),
        user(BOB, nothing),
    ];

    let mut counted_and_listed = Vec::new();
    for viewer in &viewers {
        let counted = requests.count_pending(viewer).await.unwrap();
        let page = requests
            .list(viewer, Some(RequestState::Pending), 100, "")
            .await
            .unwrap();
        let listed = page.requests.len() + page.followed_shows.len();
        counted_and_listed.push((counted, u32::try_from(listed).unwrap()));
    }

    // Ada's Pending Movie, Followed Show and its two Season Requests.
    assert_eq!(counted_and_listed, [(4, 4), (4, 4), (0, 0)]);
}

/// A Shows Fulfiller that takes every Request, and keeps the TMDB id of each Show it is told
/// to stop following.
#[derive(Default)]
struct Unfollowing(std::sync::Mutex<Vec<i64>>);

impl Fulfiller<Shows> for Unfollowing {
    fn fulfill<'a>(&'a self, _: &'a Request) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        Box::pin(async { Ok(fulfilled()) })
    }

    fn unfollow<'a>(&'a self, season: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        self.0
            .lock()
            .unwrap()
            .push(asked_season(season).show_tmdb_id);
        Box::pin(async { Ok(()) })
    }

    fn progress<'a>(
        &'a self,
        _: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// The TMDB ids of the Shows `unfollowing` was told to stop following, once it was told any,
/// waiting a second at most.
async fn unfollowed_shows(unfollowing: &Unfollowing) -> Vec<i64> {
    let told = async {
        while unfollowing.0.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    };
    tokio::time::timeout(Duration::from_secs(1), told)
        .await
        .expect("the Shows Fulfiller was never told to stop following a Show");
    unfollowing.0.lock().unwrap().clone()
}

/// Requests whose Shows Fulfiller is the returned [`Unfollowing`], with Ada following Dark,
/// Approved. Returns the Followed Show's id too.
async fn following_dark() -> (tempfile::TempDir, Requests, Arc<Unfollowing>, i64) {
    let unfollowing = Arc::new(Unfollowing::default());
    let fulfiller = Arc::clone(&unfollowing);
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(FulfillerPlugin("shows", fulfiller)))
        .unwrap();
    let (data, requests) = approved_on::<Movies>(&plugins, &[], Arc::new(Asked::default())).await;
    let followed = follow_as(&requests.db, ADA, dark(), RequestState::Approved).await;
    let id = followed.value.0.id;
    (data, requests, unfollowing, id)
}

#[tokio::test]
async fn a_followed_show_cancelled_by_a_caller_who_left_is_published_and_unfollowed() {
    let (_data, requests, unfollowing, id) = following_dark().await;
    let mut published = subscribe(&requests);
    let cancelling = requests.clone();

    leave_once_queued(
        &requests,
        async move { cancelling.cancel_followed(id).await },
    )
    .await;

    let cancelled = next_happened(&mut published).await;
    assert!(
        matches!(cancelled, Happened::FollowedShowCancelled(_)),
        "{cancelled:?}"
    );
    assert_eq!(unfollowed_shows(&unfollowing).await, [dark().show_tmdb_id]);
}

#[tokio::test]
async fn a_followed_show_withdrawn_from_by_its_last_requester_who_left_is_published_and_unfollowed()
{
    let (_data, requests, unfollowing, id) = following_dark().await;
    let mut published = subscribe(&requests);
    let withdrawing = requests.clone();

    leave_once_queued(&requests, async move {
        withdrawing
            .withdraw_followed(&ada(Permissions::NEW_USER), id)
            .await
    })
    .await;

    let withdrawn = next_happened(&mut published).await;
    assert!(
        matches!(withdrawn, Happened::FollowedShowWithdrawn(_)),
        "{withdrawn:?}"
    );
    assert_eq!(unfollowed_shows(&unfollowing).await, [dark().show_tmdb_id]);
}

#[tokio::test]
async fn upgrading_makes_a_season_a_scan_moved_before_hand_off_pending_and_leaves_handed_off_and_followed_shows_seasons()
 {
    let data_dir = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(data_dir.path().join(crate::db::FILE_NAME))
        .create_if_missing(true);
    // A raw connection, not `Database::write`: `Database::open` would run every migration.
    let mut before = sqlx::ConnectOptions::connect(&options).await.unwrap();
    // The last migration before stuck Requests went back to Pending.
    sqlx::migrate!().run_to(38, &mut before).await.unwrap();
    sqlx::query(
        "INSERT INTO followed_shows (id, tmdb_id, title, state, created_at)
         VALUES (1, 70523, 'Dark', 'approved', 0);
         INSERT INTO requests (id, tmdb_id, season, title, state, created_at,
                               fulfiller_reference, followed_show_id)
         VALUES (1, 70523, 1, 'Dark', 'partially_available', 0, NULL, NULL),
                (2, 70523, 2, 'Dark', 'partially_available', 0, 'show-70523-season-2', NULL),
                (3, 70523, 3, 'Dark', 'partially_available', 0, NULL, 1)",
    )
    .execute(&mut before)
    .await
    .unwrap();
    sqlx::Connection::close(before).await.unwrap();

    let db = Database::open(data_dir.path()).await.unwrap();
    let states: Vec<(i64, String)> = sqlx::query_as("SELECT id, state FROM requests ORDER BY id")
        .fetch_all(db.reader())
        .await
        .unwrap();

    assert_eq!(
        states,
        [
            (1, "pending".to_owned()),
            (2, "partially_available".to_owned()),
            (3, "partially_available".to_owned())
        ]
    );
}

/// Dark's Season Requests as a later read shows them, oldest first: each one's Season, state
/// and Requesters, first asker first.
async fn dark_seasons(db: &Database) -> Vec<(u32, RequestState, Vec<i64>)> {
    let newest_first = db.write(|tx| Box::pin(load(tx, None, None))).await.unwrap();
    newest_first
        .into_iter()
        .rev()
        .map(|request| {
            let requesters = request.requesters.iter().map(|one| one.user_id).collect();
            let state = request.state.as_known().unwrap_or_default();
            (asked_season(&request).season_number, state, requesters)
        })
        .collect()
}

/// The Season Requests for Seasons 1 and 2 of Dark that Ada following it in `state` opens, as
/// [`dark_seasons`] reads them.
fn opened_by_ada_following_dark(state: RequestState) -> [(u32, RequestState, Vec<i64>); 2] {
    [1, 2].map(|season| (season, state, vec![ADA]))
}

#[tokio::test]
async fn a_scan_opens_a_new_season_of_an_approved_followed_show_for_its_requesters_in_the_order_they_asked()
 {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, BOB, dark(), RequestState::Approved).await;
    follow_as(&db, ADA, dark(), RequestState::Pending).await;
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), [("created", RequestState::Available)]);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Available, vec![BOB, ADA]))
    );
}

#[tokio::test]
async fn a_season_a_scan_opens_is_partially_available_or_available_by_its_matched_aired_episodes() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    match_dark_episodes(&db, 3, &[1]).await;
    match_dark_episodes(&db, 4, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3, 4]).await;

    assert_eq!(
        said(&scanned),
        [
            ("created", RequestState::PartiallyAvailable),
            ("created", RequestState::Available)
        ]
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_without_matched_episodes() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await,
        opened_by_ada_following_dark(RequestState::Approved)
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_for_a_followed_show_not_yet_approved() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Pending).await;
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await,
        opened_by_ada_following_dark(RequestState::Pending)
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_the_followed_show_already_covers() {
    let (_data, db) = ada_and_bob().await;
    let followed = follow_as(&db, ADA, dark(), RequestState::Approved).await;
    for season in season_ids(&followed) {
        db.write(move |tx| Box::pin(cancel(tx, season)))
            .await
            .unwrap();
    }
    match_dark_episodes(&db, 1, &[1, 2]).await;
    match_dark_episodes(&db, 2, &[1, 2]).await;

    let scanned = scan_dark(&db, &[1, 2]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(dark_seasons(&db).await, []);
}

#[tokio::test]
async fn a_scan_opens_no_season_that_was_once_available() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let season_3 = WantedSeasons {
        seasons: vec![dark_season(3)],
        ..dark()
    };
    db.write(|tx| Box::pin(ask_seasons(tx, BOB, season_3, RequestState::Pending)))
        .await
        .unwrap();
    match_dark_episodes(&db, 3, &[1, 2]).await;
    scan_dark(&db, &[3]).await;

    let scanned_again = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned_again), []);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Available, vec![BOB]))
    );
}

#[tokio::test]
async fn a_season_a_scan_opened_does_not_open_again_once_cancelled() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    match_dark_episodes(&db, 3, &[1]).await;
    let opened = scan_dark(&db, &[3]).await;
    let Some(Happened::RequestCreated(created)) = opened.first().and_then(|one| one.event.as_ref())
    else {
        panic!("the scan opened no Request: {opened:?}");
    };
    let season_3 = created.request.id;
    db.write(move |tx| Box::pin(cancel(tx, season_3)))
        .await
        .unwrap();

    let scanned_again = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned_again), []);
    assert_eq!(
        dark_seasons(&db).await,
        opened_by_ada_following_dark(RequestState::Approved)
    );
}

/// Ada follows Dark while Seasons 1 and 2 are out, Pending, then Bob joins her Followed Show
/// once Season 3 has aired. Returns the Followed Show's id and Season 3's Request id.
async fn bob_joins_following_dark_after_season_3_aired(db: &Database) -> (i64, i64) {
    follow_as(db, ADA, dark(), RequestState::Pending).await;
    let mut to_season_3 = dark();
    to_season_3.seasons.push(dark_season(3));
    let joined = follow_as(db, BOB, to_season_3, RequestState::Pending).await;
    let season_3 = joined.value.1.last().expect("Season 3's Request");
    assert_eq!(asked_season(season_3).season_number, 3);
    (joined.value.0.id, season_3.id)
}

/// Approves the Pending Followed Show `id`, and with it its Pending Season Requests.
async fn approve_followed(db: &Database, id: i64) {
    db.write(move |tx| Box::pin(decide_followed(tx, id, RequestState::Approved, None, None)))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_scan_opens_no_season_asked_for_by_joining_a_followed_show_and_declined() {
    let (_data, db) = ada_and_bob().await;
    let (followed, season_3) = bob_joins_following_dark_after_season_3_aired(&db).await;
    db.write(move |tx| Box::pin(decline(tx, season_3, None)))
        .await
        .unwrap();
    approve_followed(&db, followed).await;
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Declined, vec![BOB]))
    );
}

#[tokio::test]
async fn a_scan_opens_no_season_asked_for_by_joining_a_followed_show_and_cancelled_once_approved() {
    let (_data, db) = ada_and_bob().await;
    let (followed, season_3) = bob_joins_following_dark_after_season_3_aired(&db).await;
    approve_followed(&db, followed).await;
    db.write(move |tx| Box::pin(cancel(tx, season_3)))
        .await
        .unwrap();
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), []);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(2, RequestState::Approved, vec![ADA, BOB]))
    );
}

#[tokio::test]
async fn asking_for_a_season_on_its_own_leaves_which_seasons_the_followed_show_covers() {
    let (_data, db) = ada_and_bob().await;
    follow_as(&db, ADA, dark(), RequestState::Approved).await;
    let season_3 = WantedSeasons {
        seasons: vec![dark_season(3)],
        ..dark()
    };
    let asked = db
        .write(|tx| Box::pin(ask_seasons(tx, BOB, season_3, RequestState::Pending)))
        .await
        .unwrap();
    let asked_id = asked.value[0].id;
    db.write(move |tx| Box::pin(decline(tx, asked_id, None)))
        .await
        .unwrap();
    match_dark_episodes(&db, 3, &[1, 2]).await;

    let scanned = scan_dark(&db, &[3]).await;

    assert_eq!(said(&scanned), [("created", RequestState::Available)]);
    assert_eq!(
        dark_seasons(&db).await.last(),
        Some(&(3, RequestState::Available, vec![ADA]))
    );
}
