use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;
use nama_plugin_api::{
    BoxFuture, PluginError,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::{
    ListRequestsResponse, Permission, Request, RequestState, event::Event as Happened,
};
use tokio::sync::Notify;

use super::apply::Applied;
use super::listing::{load, load_one};
use super::testing::{
    ADA, Asked, BOB, DARK, FulfillerPlugin, NOTHING, ada, ada_and_bob, approved_for, approved_on,
    ask_as, ask_each, ask_for, asked_season, breaking_bad, come_up, dark, dark_season, downloading,
    episode, follow_as, fulfilled, handed_off, happened, insert_in_each_state, match_dark_episodes,
    match_in_a_library, matched, movie, next_happened, requests_on, said, scan_dark, season_1,
    season_ids, settled, tmdb, user,
};
use super::{
    Error, Requests, Wanted, WantedSeasons, approve, ask_movie, ask_seasons, asked, availability,
    cancel, cancel_followed, decide_followed, decline, progress, qualities, retry, withdraw,
    withdraw_followed,
};
use crate::{
    db::Database,
    events::{EventBus, Filter},
    libraries::{Kind, Libraries},
    library::contents::{Change, Contents, Decisions, Item},
    plugins::{ExtensionPoints, Registry},
    server_settings::ServerUrl,
    users::{Permissions, User},
};

const TODAY: &str = "2026-11-08";

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

/// Requests in a new database, which holds an Approved Movie Request for each of `tmdb_ids`,
/// and whose one Plugin provides the returned [`Gated`] as the Movie Fulfiller.
async fn approved_for_gated(tmdb_ids: &[i64]) -> (tempfile::TempDir, Requests, Arc<Gated>) {
    let gated = Arc::new(Gated::default());
    let fulfiller = Arc::clone(&gated);
    let (data, requests) = approved_for(tmdb_ids, fulfiller).await;
    (data, requests, gated)
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

/// The id of the Season Request whose Show `applied` stops following, if any.
fn unfollowed<T>(applied: &Applied<T>) -> Option<i64> {
    applied.unfollow.as_ref().map(|season| season.id)
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
