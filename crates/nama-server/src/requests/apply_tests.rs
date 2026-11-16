use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};

use futures_util::Stream;
use nama_plugin_api::{
    BoxFuture, PluginError, contract,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
    metadata::ExternalId,
};
use nama_plugin_tmdb::testing::FakeTmdb;
use nama_proto::nama::v1::{RequestState, SubscribeResponse, event::Event as Happened};
use tokio::sync::Notify;

use crate::requests::Requests;
use crate::requests::testing::{
    ADA, Asked, DARK, FulfillerPlugin, ada, approved_for, approved_on, ask_as, dark, enable_tmdb,
    follow_as, fulfilled, next_happened, told_season,
};
use crate::{
    events::Filter,
    images::ImageStore,
    library::artwork::Artwork,
    library::contents::{Change, Item},
    library::libraries::{Kind, Libraries},
    library::scan_locks::ScanLocks,
    matching::Matcher,
    plugins::Registry,
    users::Permissions,
};

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

#[tokio::test]
async fn an_auto_approved_movie_asked_for_by_a_caller_who_left_is_published_and_handed_off() {
    let tmdb = FakeTmdb::start().await;
    let plugins = Registry::new();
    enable_tmdb(&plugins, &tmdb);
    let (_data, requests) = approved_on::<Movies>(&plugins, &[], Arc::new(Asked::default())).await;
    let mut published = subscribe(&requests);
    let asking = requests.clone();

    leave_once_queued(&requests, async move {
        asking
            .create(&ada(Permissions::ADMIN), &ExternalId::tmdb(348), None)
            .await
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
    let pinning = Matcher::new(
        Arc::clone(&requests.db),
        requests.plugins.clone(),
        Artwork::new(images, requests.plugins.clone()),
        requests.events.clone(),
        ScanLocks::for_tests(),
    );
    let mut published = subscribe(&requests);

    leave_once_queued(&requests, async move {
        pinning.set_match(item, &ExternalId::tmdb(949)).await
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

/// A Shows Fulfiller that takes every Request, and keeps the TMDB id of each Show it is told
/// to stop following.
#[derive(Default)]
struct Unfollowing(std::sync::Mutex<Vec<i64>>);

impl Fulfiller<Shows> for Unfollowing {
    fn fulfill<'a>(
        &'a self,
        _: &'a contract::Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        Box::pin(async { Ok(fulfilled()) })
    }

    fn unfollow<'a>(
        &'a self,
        season: &'a contract::Request,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let ids = &told_season(season).show_ids;
        let tmdb = ids.iter().find(|id| id.namespace == "tmdb").unwrap();
        self.0.lock().unwrap().push(tmdb.value.parse().unwrap());
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
    assert_eq!(unfollowed_shows(&unfollowing).await, [DARK]);
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
    assert_eq!(unfollowed_shows(&unfollowing).await, [DARK]);
}
