use std::{collections::HashMap, sync::Arc, time::Duration};

use nama_plugin_api::{
    BoxFuture, PluginError,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
};
use nama_proto::nama::v1::{Request, RequestState};
use tokio::sync::Notify;

use super::Claim;
use crate::plugins::Registry;
use crate::requests::Requests;
use crate::requests::asking::ask_seasons;
use crate::requests::testing::{ADA, NOTHING, approved_for, approved_on, come_up, dark};

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
