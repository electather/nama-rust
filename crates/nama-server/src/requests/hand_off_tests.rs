use std::{collections::HashMap, sync::Arc, time::Duration};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_plugin_api::{
    BoxFuture, PluginError, contract,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
};
use nama_proto::nama::v1::{RefusalReason, Request, RequestState};
use tokio::sync::Notify;

use super::Claim;
use crate::plugins::Registry;
use crate::requests::Requests;
use crate::requests::asking::{WantedSeasons, ask_seasons};
use crate::requests::testing::{ADA, NOTHING, ada, approved_for, approved_on, come_up, dark, took};
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

/// A Fulfiller for Movies and Shows that needs an id in one of the namespaces `needs` holds, and
/// keeps the ids of the Requests it is handed and of those it is told to unfollow.
#[derive(Default)]
struct Needing {
    needs: std::sync::Mutex<Vec<String>>,
    handed: std::sync::Mutex<Vec<i64>>,
    unfollowed: std::sync::Mutex<Vec<i64>>,
}

impl Needing {
    fn new(needs: &[&str]) -> Arc<Self> {
        let needing = Self::default();
        *needing.needs.lock().unwrap() = needs.iter().map(|&needed| needed.to_owned()).collect();
        Arc::new(needing)
    }

    fn handed(&self) -> Vec<i64> {
        self.handed.lock().unwrap().clone()
    }

    fn take(
        &self,
        request: &contract::Request,
    ) -> BoxFuture<'static, Result<Fulfilled, PluginError>> {
        self.handed.lock().unwrap().push(request.id);
        took("taken".to_owned())
    }
}

impl Fulfiller<Movies> for Needing {
    fn needs(&self) -> Vec<String> {
        self.needs.lock().unwrap().clone()
    }

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

impl Fulfiller<Shows> for Needing {
    fn needs(&self) -> Vec<String> {
        self.needs.lock().unwrap().clone()
    }

    fn fulfill<'a>(
        &'a self,
        request: &'a contract::Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        self.take(request)
    }

    fn unfollow<'a>(
        &'a self,
        request: &'a contract::Request,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        self.unfollowed.lock().unwrap().push(request.id);
        Box::pin(async { Ok(()) })
    }

    fn progress<'a>(
        &'a self,
        _: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// The Approved Season Requests for Dark, with the ids `show_ids` beside its tmdb id, handed to
/// `needing` as the Shows Fulfiller of the Plugin `gated`.
async fn ask_dark_of(
    needing: &Arc<Needing>,
    show_ids: ExternalIds,
) -> (tempfile::TempDir, Requests, Vec<Request>) {
    let fulfiller: Arc<dyn Fulfiller<Shows>> = Arc::<Needing>::clone(needing);
    let (data, requests) = approved_on(&Registry::new(), &[], fulfiller).await;
    let show = WantedSeasons { show_ids, ..dark() };
    let asked = requests
        .apply(|tx| Box::pin(ask_seasons(tx, ADA, show, RequestState::Approved)))
        .await
        .unwrap();
    (data, requests, asked)
}

#[tokio::test]
async fn a_request_whose_title_lacks_the_id_its_fulfiller_needs_fails_saying_which() {
    let needing = Needing::new(&["tvdb"]);

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
async fn a_request_lacking_every_id_its_fulfiller_needs_names_them_all() {
    let needing = Needing::new(&["tvdb", "imdb", "anidb"]);

    let (_data, _requests, asked) = ask_dark_of(&needing, ExternalIds::default()).await;

    assert_eq!(
        asked[0].failure_reason.as_deref(),
        Some("gated needs one of the Show's tvdb, imdb or anidb ids, and none is known for it")
    );
    assert_eq!(
        asked[0].failure.as_option().unwrap().params["namespaces"],
        "tvdb,imdb,anidb"
    );
}

#[tokio::test]
async fn a_request_with_one_id_its_fulfiller_needs_is_handed_off() {
    let needing = Needing::new(&["anidb", "tvdb"]);
    let ids = ExternalIds::from_iter([ExternalId::tvdb(81189)]);

    let (_data, _requests, asked) = ask_dark_of(&needing, ids).await;

    let handed: Vec<i64> = asked.iter().map(|request| request.id).collect();
    assert_eq!(needing.handed(), handed);
    assert!(
        asked
            .iter()
            .all(|request| request.state == RequestState::Processing
                && request.failure.as_option().is_none()),
        "{asked:?}"
    );
}

#[tokio::test]
async fn a_fulfiller_that_needs_no_id_is_handed_every_request() {
    let needing = Needing::new(&[]);
    let fulfiller: Arc<dyn Fulfiller<Movies>> = Arc::<Needing>::clone(&needing);

    let (_data, requests) = approved_for(&[949], fulfiller).await;

    come_up(&requests, "gated").await;

    assert_eq!(needing.handed().len(), 1);
}

#[tokio::test]
async fn retrying_a_request_that_lacked_an_id_forgets_why_it_failed() {
    let needing = Needing::new(&["tvdb"]);
    let (_data, requests, asked) = ask_dark_of(&needing, ExternalIds::default()).await;
    needing.needs.lock().unwrap().clear();

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
    let needing = Needing::new(&["tvdb"]);
    let (_data, requests, asked) = ask_dark_of(&needing, ExternalIds::default()).await;

    requests.hand_off.unfollow(&asked[0]).await;

    assert_eq!(*needing.unfollowed.lock().unwrap(), NOTHING);
}

#[tokio::test]
async fn a_show_with_an_id_its_fulfiller_needs_is_unfollowed() {
    let needing = Needing::new(&["tvdb"]);
    let ids = ExternalIds::from_iter([ExternalId::tvdb(81189)]);
    let (_data, requests, asked) = ask_dark_of(&needing, ids).await;

    requests.hand_off.unfollow(&asked[0]).await;

    assert_eq!(*needing.unfollowed.lock().unwrap(), [asked[0].id]);
}
