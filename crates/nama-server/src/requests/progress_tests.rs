use std::{collections::HashSet, sync::Arc, time::Duration};

use futures_util::StreamExt;
use nama_plugin_api::fulfiller::{Fulfiller, Shows};
use nama_proto::nama::v1::{Request, RequestState, event::Event as Happened};

use crate::requests::Requests;
use crate::requests::asking::{WantedSeasons, ask_seasons};
use crate::requests::testing::{
    ADA, Asked, FulfillerPlugin, ada, ada_and_bob, approved_for, ask_as, come_up, dark,
    dark_season, downloading, insert_in_each_state, match_dark_episodes, match_in_a_library,
    next_happened, scan_dark,
};
use crate::{
    db::Database,
    events::{EventBus, Filter},
    plugins::{self, Registry},
    server_settings::ServerUrl,
    users::Permissions,
};

/// The interval a test polls progress at: how long a poll waits for the fake Fulfillers, which
/// answer at once.
const INTERVAL: Duration = Duration::from_secs(5);

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
    let fulfilling: HashSet<i64> = super::fulfilling_sql!(read)
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
        let expected = super::is_fulfilling(&request);
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
    let shown: Vec<_> = (downloading("season-1").into_iter())
        .map(plugins::download)
        .collect();
    assert_eq!(published.downloads, shown);
    assert_eq!(partly.downloads, shown);
    assert!(!any_once_available);
    assert_eq!(asked.batches().len(), batches);
    assert_eq!(published_once_available.ok(), None);
    assert_eq!(available.state, RequestState::Available);
    assert_eq!(available.downloads, []);
}
