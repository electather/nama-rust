use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;
use nama_plugin_api::{
    BoxFuture, PluginError,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
};
use nama_proto::nama::v1::{
    ListRequestsResponse, Permission, Request, RequestState, event::Event as Happened,
};
use tokio::sync::Notify;

use super::apply::Applied;
use super::asking::{WantedSeasons, ask_seasons};
use super::testing::{
    ADA, Asked, BOB, FulfillerPlugin, NOTHING, ada, ada_and_bob, approved_for, approved_on, ask_as,
    ask_for, come_up, dark, dark_season, downloading, follow_as, handed_off, insert_in_each_state,
    match_dark_episodes, match_in_a_library, next_happened, requests_on, said, scan_dark, user,
};
use super::{Error, Requests, asked, progress};
use crate::{
    db::Database,
    events::{EventBus, Filter},
    plugins::{ExtensionPoints, Registry},
    server_settings::ServerUrl,
    users::{Permissions, User},
};

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
