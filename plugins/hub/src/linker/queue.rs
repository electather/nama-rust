//! The queue each kind of data the Hub Plugin sends the Hub waits in, kept in the Plugin's Store so
//! what the Hub can't take now goes once it is back (ADR 0008). A kind supplies only its key, by
//! which the latest of each item wins across the whole queue, and the Hub call that sends a page.

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    mem,
    sync::Arc,
};

use connectrpc::{ConnectError, ErrorCode};
use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{
        Kernel,
        functions::{StoreDelete, StoreGet, StorePut},
    },
    contract::StoreEntry,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;
use tracing::warn;

use super::{credential, store_key};

/// The most bytes the Plugin Store keeps in one value: the Kernel refuses a longer one, and a
/// Plugin can't read the Kernel's own constant (ADR 0006).
pub(crate) const MAX_VALUE_BYTES: usize = 64 * 1024;

/// A kind of data queued for the Hub.
pub(crate) trait Queued: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// The Store key of the queue's header; its pages are kept under `<STORE_KEY>/<n>`.
    const STORE_KEY: &'static str;
    /// The most items one Hub call takes, and so a page holds.
    const PER_CALL: usize;
    /// The one Store value an older Hub Plugin queued this kind in, taken in once.
    const OLD_STORE_KEY: Option<&'static str> = None;

    /// Names the item apart from others: of two with one key, only the later is sent.
    fn key(&self) -> impl Eq + Hash + '_;

    /// When it was made, in milliseconds since the Unix epoch.
    fn changed_at_ms(&self) -> i64;

    /// The items [`Self::OLD_STORE_KEY`]'s value holds, oldest first.
    fn from_old(old: &[u8]) -> Result<Vec<Self>, PluginError> {
        Ok(serde_json::from_slice(old)?)
    }
}

/// Sends a page to the Hub with the Server's credential.
type SendPage<T> =
    Box<dyn Fn(String, Vec<T>) -> BoxFuture<'static, Result<(), PluginError>> + Send + Sync>;

/// Which pages the Store keeps: `first..next`, oldest first.
#[derive(Default, Serialize, Deserialize)]
struct Header {
    first: u64,
    next: u64,
}

/// One kind's queue.
pub(crate) struct Queue<T> {
    kernel: Arc<dyn Kernel>,
    send_page: SendPage<T>,
    /// Held while the queue is read and written, so nothing queued meanwhile is lost.
    held: Mutex<()>,
}

impl<T: Queued> Queue<T> {
    /// The queue in `kernel`'s Store, whose pages `send_page` sends.
    pub(crate) fn new(
        kernel: Arc<dyn Kernel>,
        send_page: impl Fn(String, Vec<T>) -> BoxFuture<'static, Result<(), PluginError>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            kernel,
            send_page: Box::new(send_page),
            held: Mutex::default(),
        }
    }

    /// Queues `batch`, oldest first. An item replaces a queued one with its key, going last,
    /// unless that one was made later, when the item is dropped.
    pub(crate) async fn add(&self, batch: Vec<T>) -> Result<(), PluginError> {
        let _held = self.held.lock().await;
        self.take_old().await?;
        self.add_held(batch).await
    }

    /// Sends the queue, a page per Hub call, oldest first, forgetting each page once it is sent.
    /// The Hub refusing a page as invalid drops it, as it would refuse it again, and not knowing
    /// the Server's credential counts as sent, as no User is Linked through it. Without the
    /// Server's Link the queue has nowhere to go, and is forgotten. Any other failure, such as the
    /// Hub being down, keeps the page and the rest, and is the error.
    pub(crate) async fn send(&self) -> Result<(), PluginError> {
        let _held = self.held.lock().await;
        self.take_old().await?;
        let mut header = self.header().await?;
        let credential = credential(self.kernel.as_ref()).await?;
        while header.first < header.next {
            let page = page_key::<T>(header.first);
            if let Some(credential) = &credential {
                let items = self.stored_json(&page).await?.unwrap_or_default();
                sent::<T>((self.send_page)(credential.clone(), items).await)?;
            }
            header.first += 1;
            self.put_json(T::STORE_KEY, &header).await?;
            self.delete(&page).await?;
        }
        Ok(())
    }

    /// What `pull` answers with the Server's credential once the queue is sent, so it is never
    /// older than what this Server queued; the default without the Server's Link.
    pub(crate) async fn pull<R: Default>(
        &self,
        pull: impl AsyncFnOnce(&str) -> Result<R, PluginError>,
    ) -> Result<R, PluginError> {
        self.send().await?;
        match credential(self.kernel.as_ref()).await? {
            Some(credential) => pull(&credential).await,
            None => Ok(R::default()),
        }
    }

    async fn add_held(&self, batch: Vec<T>) -> Result<(), PluginError> {
        if batch.is_empty() {
            return Ok(());
        }
        let old = self.header().await?;
        // ponytail: every batch reads and rewrites the whole queue, which is short while its Job
        // drains it every few minutes and grows only while the Hub is down; past a few hundred
        // pages, keep an index of the queued keys beside the header instead.
        let mut items = Vec::new();
        for nth in old.first..old.next {
            items.extend(
                self.stored_json::<Vec<T>>(&page_key::<T>(nth))
                    .await?
                    .unwrap_or_default(),
            );
        }
        items.extend(batch);
        // Numbered after the old pages, which go only once the header names the new ones, so a
        // failure midway leaves the queue as it was.
        let mut new = Header {
            first: old.next,
            next: old.next,
        };
        for page in pages::<T>(&latest_of_each(items))? {
            self.put(page_key::<T>(new.next), page).await?;
            new.next += 1;
        }
        self.put_json(T::STORE_KEY, &new).await?;
        for nth in old.first..old.next {
            self.delete(&page_key::<T>(nth)).await?;
        }
        Ok(())
    }

    /// Takes in the value an older Hub Plugin queued this kind in, if it is still there.
    async fn take_old(&self) -> Result<(), PluginError> {
        let Some(old_store_key) = T::OLD_STORE_KEY else {
            return Ok(());
        };
        let Some(old) = self.stored(old_store_key).await? else {
            return Ok(());
        };
        self.add_held(T::from_old(&old)?).await?;
        self.delete(old_store_key).await
    }

    async fn header(&self) -> Result<Header, PluginError> {
        Ok(self.stored_json(T::STORE_KEY).await?.unwrap_or_default())
    }

    async fn stored(&self, key: &str) -> Result<Option<Vec<u8>>, PluginError> {
        Ok(self.kernel.ask::<StoreGet>(store_key(key)).await?.value)
    }

    async fn stored_json<V: DeserializeOwned>(&self, key: &str) -> Result<Option<V>, PluginError> {
        match self.stored(key).await? {
            Some(stored) => Ok(Some(serde_json::from_slice(&stored)?)),
            None => Ok(None),
        }
    }

    async fn put(&self, key: String, value: Vec<u8>) -> Result<(), PluginError> {
        (self.kernel)
            .ask::<StorePut>(StoreEntry {
                key,
                value,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    async fn put_json(&self, key: &str, value: &impl Serialize) -> Result<(), PluginError> {
        self.put(key.to_owned(), serde_json::to_vec(value)?).await
    }

    async fn delete(&self, key: &str) -> Result<(), PluginError> {
        self.kernel.ask::<StoreDelete>(store_key(key)).await?;
        Ok(())
    }
}

/// `sending` a page, done unless the Hub couldn't take it now.
fn sent<T: Queued>(sending: Result<(), PluginError>) -> Result<(), PluginError> {
    let Err(err) = sending else {
        return Ok(());
    };
    match err
        .downcast_ref::<ConnectError>()
        .map(|refused| refused.code)
    {
        Some(ErrorCode::Unauthenticated) => Ok(()),
        Some(ErrorCode::InvalidArgument) => {
            warn!(
                "the Hub refused a page of {} as invalid, so it was dropped: {err}",
                T::STORE_KEY
            );
            Ok(())
        }
        _ => Err(err),
    }
}

/// `items`, oldest first, keeping of each key only the one made last, where it stands; of two made
/// at once, the later.
fn latest_of_each<T: Queued>(items: Vec<T>) -> Vec<T> {
    let mut latest = HashMap::new();
    for (nth, item) in items.iter().enumerate() {
        let at = item.changed_at_ms();
        let kept = latest.entry(item.key()).or_insert((nth, at));
        if kept.1 <= at {
            *kept = (nth, at);
        }
    }
    let keep: HashSet<usize> = latest.into_values().map(|(nth, _)| nth).collect();
    (items.into_iter().enumerate())
        .filter_map(|(nth, item)| keep.contains(&nth).then_some(item))
        .collect()
}

/// `items` in pages as the Store keeps them, each a JSON array of at most [`Queued::PER_CALL`]
/// items and [`MAX_VALUE_BYTES`].
fn pages<T: Queued>(items: &[T]) -> Result<Vec<Vec<u8>>, PluginError> {
    let (mut pages, mut page, mut in_page) = (Vec::new(), Vec::new(), 0);
    for item in items {
        let item = serde_json::to_vec(item)?;
        // `page` holds `[` and its items so far: this one adds a comma and itself, and `]` closes.
        if in_page == T::PER_CALL
            || (in_page > 0 && page.len() + 1 + item.len() + 1 > MAX_VALUE_BYTES)
        {
            page.push(b']');
            pages.push(mem::take(&mut page));
            in_page = 0;
        }
        page.push(if in_page == 0 { b'[' } else { b',' });
        page.extend(item);
        in_page += 1;
    }
    if in_page > 0 {
        page.push(b']');
        pages.push(page);
    }
    Ok(pages)
}

/// The Store key of `T`'s page `nth`.
fn page_key<T: Queued>(nth: u64) -> String {
    format!("{}/{nth}", T::STORE_KEY)
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
