use std::{collections::HashMap, sync::Arc, time::Duration};

use nama_plugin_api::metadata::ExternalIds;
use nama_plugin_api::{
    BoxFuture, PluginError, PluginId, contract,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
};
use nama_proto::nama::v1::{RefusalReason, Request, RequestState};
use tokio::sync::Notify;

use super::Claim;
use crate::plugins::{Registry, ScriptedFulfiller};
use crate::requests::Requests;
use crate::requests::asking::{WantedSeasons, ask_seasons};
use crate::requests::testing::{
    ADA, FulfillerPlugin, NOTHING, ada, approved_for, approved_on, come_up, dark, read,
};
use crate::users::Permissions;

#[test]
fn a_claim_another_hand_off_skipped_is_kept_for_one_more_look() {
    let handing = Arc::default();
    let claim = Claim::take(&handing, 1).unwrap();
    assert!(Claim::take(&handing, 1).is_none());

    let kept = claim.release().expect("the skipped Request was let go");

    assert!(kept.release().is_none());
    assert!(Claim::take(&handing, 1).is_some());
}

#[test]
fn a_dropped_claim_lets_the_request_be_claimed_again() {
    let handing = Arc::default();
    let claim = Claim::take(&handing, 1).unwrap();

    drop(claim);

    assert!(Claim::take(&handing, 1).is_some());
}

/// A Fulfiller for Movies and Shows that keeps the ids of the Requests it is handed, in order,
/// and takes each only once `open` lets it. Its own, since a `ScriptedFulfiller` can't hold its
/// first ask.
#[derive(Default)]
struct Gated {
    handed: std::sync::Mutex<Vec<i64>>,
    open: Notify,
}

impl Gated {
    fn handed(&self) -> Vec<i64> {
        self.handed.lock().unwrap().clone()
    }

    fn take<'a>(
        &'a self,
        request: &contract::Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
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
        request: &'a contract::Request,
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
        request: &'a contract::Request,
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

/// The Approved Season Requests for Dark, with the ids `show_ids` beside its tmdb id, handed to
/// `fulfiller` as the Shows Fulfiller of the Plugin `gated`.
async fn ask_dark_of(
    fulfiller: &Arc<ScriptedFulfiller>,
    show_ids: ExternalIds,
) -> (tempfile::TempDir, Requests, Vec<Request>) {
    let shows: Arc<dyn Fulfiller<Shows>> = Arc::<ScriptedFulfiller>::clone(fulfiller);
    let (data, requests) = approved_on(&Registry::new(), &[], shows).await;
    let show = WantedSeasons { show_ids, ..dark() };
    let asked = requests
        .apply(|tx| Box::pin(ask_seasons(tx, ADA, show, RequestState::Approved)))
        .await
        .unwrap();
    (data, requests, asked)
}

/// A Fulfiller that needs a tvdb id.
fn needing_tvdb() -> Arc<ScriptedFulfiller> {
    Arc::new(ScriptedFulfiller::default().needing(&["tvdb"]))
}

#[tokio::test]
async fn a_request_whose_title_lacks_the_id_its_fulfiller_needs_fails_saying_which() {
    let needing = needing_tvdb();

    let (_data, _requests, asked) = ask_dark_of(&needing, ExternalIds::default()).await;

    assert_eq!(needing.handed(), NOTHING);
    let failed = &asked[0];
    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("gated needs the Show's tvdb id, and none is known for it")
    );
    let failure = failed.failure.as_option().expect("no typed failure");
    assert_eq!(
        failure.reason.as_known(),
        Some(RefusalReason::REFUSAL_REASON_TITLE_ID_MISSING)
    );
    assert_eq!(failure.params["namespaces"], "tvdb");
    assert_eq!(failure.params["pluginId"], "gated");
}

#[tokio::test]
async fn retrying_a_request_that_lacked_an_id_forgets_why_it_failed() {
    let needing = needing_tvdb();
    let (_data, requests, asked) = ask_dark_of(&needing, ExternalIds::default()).await;
    needing.need(&[]);

    let retried = requests
        .retry(&ada(Permissions::ADMIN), asked[0].id, None)
        .await
        .unwrap();

    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(retried.failure_reason, None);
    assert!(retried.failure.as_option().is_none(), "{retried:?}");
}

#[tokio::test]
async fn a_show_lacking_every_id_its_fulfiller_needs_isnt_unfollowed() {
    let needing = needing_tvdb();
    let (_data, requests, asked) = ask_dark_of(&needing, ExternalIds::default()).await;

    requests.hand_off.unfollow(&asked[0]).await;

    assert_eq!(needing.unfollowed(), NOTHING);
}

/// How long the Fulfiller says to wait when it says to try again.
const TRY_AGAIN_IN: Duration = Duration::from_secs(30);

/// A Movies Fulfiller that says to try again in [`TRY_AGAIN_IN`] the first time it is handed a
/// Request.
fn trying_again_once() -> Arc<ScriptedFulfiller> {
    Arc::new(ScriptedFulfiller::default().trying_again(1, TRY_AGAIN_IN))
}

/// The id of the one Request [`approved_for`] opens in a new database.
const FIRST: i64 = 1;

/// The [`FIRST`] Request once it is no longer Approved.
async fn moved_on(requests: &Requests) -> Request {
    loop {
        let request = read(&requests.db, FIRST).await;
        if request.state != RequestState::Approved {
            return request;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_request_its_fulfiller_says_to_try_again_is_handed_off_again_later() {
    let busy = trying_again_once();
    let (_data, requests) = approved_for(&[949], Arc::<ScriptedFulfiller>::clone(&busy)).await;
    // After opening the database, whose pool times out by the clock.
    tokio::time::pause();

    come_up(&requests, "gated").await;
    let asked_by_come_up = busy.asked();
    tokio::time::advance(TRY_AGAIN_IN).await;
    // Real time again: the hand-off reads and writes SQLite, which a paused clock would race.
    tokio::time::resume();
    let handed_again = tokio::time::timeout(Duration::from_secs(5), moved_on(&requests)).await;

    assert_eq!(asked_by_come_up, 1);
    // Handed off again only if it stayed Approved, not Failed.
    let handed_again = handed_again.expect("it was never handed off again");
    assert_eq!(handed_again.state, RequestState::Processing);
    assert_eq!(busy.asked(), 2);
}

#[tokio::test]
async fn disabling_the_fulfiller_before_it_is_asked_again_leaves_the_request_waiting() {
    let plugins = Registry::new();
    let busy = trying_again_once();
    let fulfiller: Arc<dyn Fulfiller<Movies>> = Arc::<ScriptedFulfiller>::clone(&busy);
    let (_data, requests) = approved_on(&plugins, &[949], Arc::clone(&fulfiller)).await;
    tokio::time::pause();
    come_up(&requests, "gated").await;

    assert!(plugins.disable(&PluginId::from_static("gated")).await);
    // Enabled again, so a retry that outlived the disable would find a Fulfiller to hand off to.
    plugins
        .enable(Arc::new(FulfillerPlugin("gated", fulfiller)))
        .unwrap();
    tokio::time::sleep(TRY_AGAIN_IN * 2).await;
    let request = read(&requests.db, FIRST).await;
    tokio::time::resume();

    assert_eq!(request.state, RequestState::Approved);
    assert_eq!(busy.asked(), 1);
}
