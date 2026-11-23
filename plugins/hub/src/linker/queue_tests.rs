use std::sync::{
    Mutex as StdMutex,
    atomic::{AtomicBool, Ordering},
};

use nama_plugin_api::automation::functions::StorePut;

use super::*;
use crate::tests::store;

/// An item of a kind made up for these tests, a page of which holds at most 3.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Told {
    key: String,
    at: i64,
    padding: String,
}

impl Queued for Told {
    const STORE_KEY: &'static str = "told";
    const PER_CALL: usize = 3;
    const OLD_STORE_KEY: Option<&'static str> = Some("told-before");

    fn key(&self) -> impl Eq + Hash + '_ {
        &self.key
    }

    fn changed_at_ms(&self) -> i64 {
        self.at
    }
}

fn told(key: &str, at: i64) -> Told {
    Told {
        key: key.to_owned(),
        at,
        padding: String::new(),
    }
}

/// What the Hub took: each page sent, as its items' keys and times.
type Taken = Arc<StdMutex<Vec<Vec<(String, i64)>>>>;

/// How the Hub answers the next calls, in order, before it takes every page after.
type Refusing = Arc<StdMutex<Vec<ErrorCode>>>;

struct Hub {
    queue: Queue<Told>,
    kernel: Arc<dyn Kernel>,
    taken: Taken,
    refusing: Refusing,
    linked: AtomicBool,
}

impl Hub {
    /// A queue over an empty Store, the Server Linked.
    fn linked() -> Self {
        let hub = Self::not_linked();
        hub.link();
        hub
    }

    fn not_linked() -> Self {
        let kernel = store();
        let (taken, refusing) = (Taken::default(), Refusing::default());
        Self {
            queue: Queue::new(Arc::clone(&kernel)),
            kernel,
            taken,
            refusing,
            linked: AtomicBool::new(false),
        }
    }

    fn link(&self) {
        self.linked.store(true, Ordering::Relaxed);
    }

    /// Sends the queue, to the Hub if the Server is Linked.
    async fn send(&self) -> Result<(), PluginError> {
        (self.queue)
            .send(|page| {
                let answered = if self.linked.load(Ordering::Relaxed) {
                    answer(&self.taken, &self.refusing, page).map(Some)
                } else {
                    Ok(None)
                };
                Box::pin(std::future::ready(answered))
            })
            .await
    }

    async fn put(&self, key: &str, value: Vec<u8>) {
        let entry = StoreEntry {
            key: key.to_owned(),
            value,
            ..Default::default()
        };
        self.kernel.ask::<StorePut>(entry).await.unwrap();
    }

    fn refuse(&self, answers: &[ErrorCode]) {
        self.refusing.lock().unwrap().extend_from_slice(answers);
    }

    fn taken(&self) -> Vec<Vec<(String, i64)>> {
        self.taken.lock().unwrap().clone()
    }
}

/// The Hub's answer to `page`: the next of `refusing`, or taking it into `taken` once there is none.
fn answer(taken: &Taken, refusing: &Refusing, page: Vec<Told>) -> Result<(), PluginError> {
    let mut refusing = refusing.lock().unwrap();
    if !refusing.is_empty() {
        return Err(ConnectError::new(refusing.remove(0), "refused").into());
    }
    let page = page.into_iter().map(|told| (told.key, told.at)).collect();
    taken.lock().unwrap().push(page);
    Ok(())
}

/// `pages` of `(key, at)`, as [`Hub::taken`] answers them.
fn pages(pages: &[&[(&str, i64)]]) -> Vec<Vec<(String, i64)>> {
    (pages.iter())
        .map(|page| page.iter().map(|&(key, at)| (key.to_owned(), at)).collect())
        .collect()
}

#[tokio::test]
async fn items_go_in_order_a_page_per_call_the_latest_of_each_key_last_across_pages() {
    let hub = Hub::linked();

    let first: Vec<_> = ["a", "b", "c", "d", "e"].map(|key| told(key, 1)).into();
    hub.queue.add(first).await.unwrap();
    hub.queue.add(vec![told("b", 2)]).await.unwrap();
    hub.send().await.unwrap();

    assert_eq!(
        hub.taken(),
        pages(&[&[("a", 1), ("c", 1), ("d", 1)], &[("e", 1), ("b", 2)]])
    );
}

#[tokio::test]
async fn an_item_told_after_a_later_one_of_its_key_is_dropped() {
    let hub = Hub::linked();

    hub.queue
        .add(vec![told("a", 2), told("b", 1)])
        .await
        .unwrap();
    hub.queue.add(vec![told("a", 1)]).await.unwrap();
    hub.send().await.unwrap();

    assert_eq!(hub.taken(), pages(&[&[("a", 2), ("b", 1)]]));
}

#[tokio::test]
async fn telling_the_same_items_again_queues_each_once() {
    let hub = Hub::linked();
    let all: Vec<_> = (0..7).map(|key| told(&key.to_string(), 1)).collect();

    hub.queue.add(all.clone()).await.unwrap();
    hub.queue.add(all).await.unwrap();
    hub.send().await.unwrap();

    assert_eq!(hub.taken().concat().len(), 7);
}

#[tokio::test]
async fn a_page_never_outgrows_one_store_value() {
    let hub = Hub::linked();
    let big = |key: &str| Told {
        padding: "x".repeat(30_000),
        ..told(key, 1)
    };

    // The Store refuses a value over 64 KiB, so only pages of at most two fit.
    hub.queue
        .add(["a", "b", "c", "d", "e"].map(big).into())
        .await
        .unwrap();
    hub.send().await.unwrap();

    assert_eq!(
        hub.taken(),
        pages(&[&[("a", 1), ("b", 1)], &[("c", 1), ("d", 1)], &[("e", 1)]])
    );
}

#[tokio::test]
async fn a_page_the_hub_cant_take_now_stays_with_the_rest_and_fails_the_send() {
    let hub = Hub::linked();
    let four: Vec<_> = ["a", "b", "c", "d"].map(|key| told(key, 1)).into();
    hub.queue.add(four).await.unwrap();

    hub.refuse(&[ErrorCode::Unavailable]);
    assert!(hub.send().await.is_err());
    hub.send().await.unwrap();

    assert_eq!(
        hub.taken(),
        pages(&[&[("a", 1), ("b", 1), ("c", 1)], &[("d", 1)]])
    );
}

#[tokio::test]
async fn a_page_the_hub_refuses_as_invalid_is_dropped_and_the_rest_go() {
    let hub = Hub::linked();
    let four: Vec<_> = ["a", "b", "c", "d"].map(|key| told(key, 1)).into();
    hub.queue.add(four).await.unwrap();

    hub.refuse(&[ErrorCode::InvalidArgument]);
    hub.send().await.unwrap();
    hub.send().await.unwrap();

    assert_eq!(hub.taken(), pages(&[&[("d", 1)]]));
}

#[tokio::test]
async fn without_the_servers_link_the_queue_is_forgotten() {
    let hub = Hub::not_linked();
    hub.queue.add(vec![told("a", 1)]).await.unwrap();

    hub.send().await.unwrap();
    hub.link();
    hub.send().await.unwrap();

    assert_eq!(hub.taken(), pages(&[]));
}

#[tokio::test]
async fn what_an_older_plugin_queued_in_one_value_is_sent_once_before_what_is_queued_after() {
    let hub = Hub::linked();
    let before = serde_json::to_vec(&[told("a", 1), told("b", 1)]).unwrap();
    hub.put("told-before", before).await;

    hub.queue.add(vec![told("c", 1)]).await.unwrap();
    hub.send().await.unwrap();
    hub.send().await.unwrap();

    assert_eq!(hub.taken(), pages(&[&[("a", 1), ("b", 1), ("c", 1)]]));
}
