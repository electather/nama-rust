//! Linking the Server to an Account (ADR 0011): the code it shows while it waits, and the Link it
//! keeps in the Plugin's Store once someone entered it, the Server Addresses it tells the Hub, and
//! through it shares what the Admin chooses with Nama's developers (ADR 0054). [`Linker`] holds
//! the Link: it alone reads and writes it, and every Hub call made as the Server goes through
//! [`Linker::call`], so one rule decides what a Link the Hub dropped means. Its Users' Links are
//! in `users.rs`, syncing their Watch State both ways in `watch_states.rs`, their Subtitle and
//! Audio Preferences in `track_preferences.rs`, and their Watchlists in `watchlist.rs`, their plays
//! as Viewing Events in `viewing.rs`, and signing in with an Account, and Users created from one,
//! in `accounts.rs`.

mod accounts;
mod track_preferences;
mod users;
mod viewing;
mod watch_states;
mod watchlist;

use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, PoisonError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{
        Kernel,
        functions::{StoreDelete, StoreGet, StorePut},
    },
    contract::{StoreEntry, StoreKey},
    diagnostics_sharer::{DiagnosticsSharer, Signal},
    job::{JobRunner, RunJob},
    points::{TrackPreference, WatchStateChange},
    server_link::{
        AccountSignIn, Link, Reachability, ServerAddresses, ServerLinker, WatchlistEntry,
    },
};
use ring::digest;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::{
    hub::{Hub, LinkDropped, LinkedHub, Polling},
    token::{self, Claims, Rejected},
};

pub(crate) use self::{
    track_preferences::TrackPreferenceSync,
    viewing::{ViewingEvents, send_job},
    watch_states::{WatchStateSync, sync_job},
};

/// Runs the Hub Plugin's Jobs. One runner serves them all, as the Kernel asks only the first
/// `JobRunner` a Plugin provides.
pub(crate) struct Jobs {
    pub(crate) linker: Arc<Linker>,
    pub(crate) viewing: ViewingEvents,
}

impl JobRunner for Jobs {
    fn run<'a>(&'a self, run: &'a RunJob) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async move {
            match run.job_id.as_str() {
                watch_states::SYNC => {
                    // Each queue apart, so one the Hub refuses holds back only its own.
                    let watch_states = self.linker.send_queued().await;
                    let preferences = self.linker.send_queued_preferences().await;
                    watch_states.and(preferences)
                }
                viewing::SEND_JOB => self.viewing.send().await,
                other => Err(format!("the Hub Plugin has no Job {other}").into()),
            }
        })
    }
}

/// The Store key of the Link, a [`Kept`] as JSON.
const LINK_KEY: &str = "link";

/// The Store key of the Hub's key set, as the Hub publishes it.
const KEY_SET_KEY: &str = "key-set";

/// How long the Hub's check of the Server's public port stands: the same Server Addresses go to
/// the Hub again after this, so it checks again, such as after the router's public IP changed.
const RECHECK_EVERY: Duration = Duration::from_mins(30);

/// A code the Server shows, waiting for someone to enter it on the Hub.
struct Waiting {
    device_code: String,
    code: String,
    poll_every: Duration,
    next_poll: Instant,
    expires: Instant,
}

/// What the Plugin keeps of its Link.
#[derive(Serialize, Deserialize)]
struct Kept {
    /// What the Server presents on every later Hub call (ADR 0098).
    credential: String,
    /// The Account's email, as the Hub's signed token named it; `None` if the token couldn't be
    /// checked when the credential came.
    account: Option<String>,
}

impl Kept {
    /// Names this Link apart from the Server's earlier ones, without holding its credential.
    fn id(&self) -> String {
        URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, self.credential.as_bytes()))
    }
}

/// The Server Addresses the Hub has.
struct Reported {
    /// The [`Kept::id`] of the Link it got them through.
    link: String,
    addresses: ServerAddresses,
    /// When the Hub got them, and checked their public port.
    at: Instant,
}

impl Reported {
    /// Whether the Hub needs no report of `addresses` through `link` at `now`: it has them, and
    /// checked their public port less than [`RECHECK_EVERY`] ago.
    fn covers(&self, link: &str, addresses: &ServerAddresses, now: Instant) -> bool {
        self.link == link
            && self.addresses == *addresses
            && now.duration_since(self.at) < RECHECK_EVERY
    }
}

/// Who a Link whose token couldn't be checked belongs to, as Settings shows it.
const UNCONFIRMED_ACCOUNT: &str = "an Account the Hub didn't confirm";

/// The Server's Link to the Hub.
pub(crate) struct Linker {
    hub: Hub,
    kernel: Arc<dyn Kernel>,
    /// Held across a poll, so two polls never race for the one credential.
    waiting: Mutex<Option<Waiting>>,
    /// What the Plugin knows of each User's Link but what it keeps in the Store.
    users: std::sync::Mutex<HashMap<i64, users::UserState>>,
    /// Held while a queue of Watch State or Track Preference changes is read and written, so none
    /// is lost.
    queue: Mutex<()>,
    /// Held across telling the Hub, so a report never overtakes another.
    reported: Mutex<Option<Reported>>,
    /// What the Hub found of the Server's public port, with the [`Kept::id`] of the Link it found
    /// it through. Apart from `reported`, so showing the Link never waits on a report.
    reachability: std::sync::Mutex<Option<(String, Reachability)>>,
}

impl Linker {
    pub(crate) fn new(hub: Hub, kernel: Arc<dyn Kernel>) -> Self {
        Self {
            hub,
            kernel,
            waiting: Mutex::default(),
            users: std::sync::Mutex::default(),
            queue: Mutex::default(),
            reported: Mutex::default(),
            reachability: std::sync::Mutex::default(),
        }
    }

    async fn current_link(&self) -> Result<Link, PluginError> {
        let mut waiting = self.waiting.lock().await;
        if let Some(code) = waiting.as_mut() {
            match self.poll(code).await? {
                Some(showing) => return Ok(showing),
                None => *waiting = None,
            }
        }
        Ok(match self.kept().await? {
            Some(kept) => Link::Linked {
                reachability: self.reachability(&kept),
                account: kept
                    .account
                    .unwrap_or_else(|| UNCONFIRMED_ACCOUNT.to_owned()),
            },
            None => Link::NotLinked,
        })
    }

    /// Asks the Hub whether `code` was entered, once its poll interval has passed. `Some` while
    /// it still waits, `None` once it is gone: expired, or entered and its Link kept.
    async fn poll(&self, code: &mut Waiting) -> Result<Option<Link>, PluginError> {
        let now = Instant::now();
        if now >= code.expires {
            return Ok(None);
        }
        if now < code.next_poll {
            return Ok(Some(self.showing(code)));
        }
        code.next_poll = now + code.poll_every;
        let polled = match self.hub.poll(&code.device_code).await {
            Ok(polled) => polled,
            Err(Polling::Expired) => return Ok(None),
            // The code still works on the Hub, and the next poll asks again: a Hub that is down
            // for a moment shouldn't fail the Settings that show the code.
            Err(Polling::Failed) => return Ok(Some(self.showing(code))),
        };
        if !polled.linked {
            return Ok(Some(self.showing(code)));
        }
        // Kept before the token is checked: the Hub has handed it out for good, and a Link whose
        // credential was lost could never be Unlinked.
        let mut kept = Kept {
            credential: polled.credential,
            account: None,
        };
        self.keep(&kept).await?;
        kept.account = Some(self.verify(&polled.token).await?.email);
        self.keep(&kept).await?;
        Ok(None)
    }

    async fn start(&self, server_name: &str) -> Result<Link, PluginError> {
        // Held from the check on: a poll keeping a credential meanwhile would otherwise see its
        // Link overwritten by the next, leaving one on the Hub that nothing could Unlink.
        let mut waiting = self.waiting.lock().await;
        if self.kept().await?.is_some() {
            return Err("the Server is Linked already: Unlink it first".into());
        }
        let started = self.hub.start(server_name).await?;
        let now = Instant::now();
        let code = Waiting {
            device_code: started.device_code,
            code: started.user_code,
            poll_every: Duration::from_secs(started.interval_seconds.try_into().unwrap_or(5)),
            next_poll: now,
            expires: now + Duration::from_secs(started.expires_in_seconds.try_into().unwrap_or(0)),
        };
        let link = self.showing(&code);
        *waiting = Some(code);
        Ok(link)
    }

    async fn forget_link(&self) -> Result<(), PluginError> {
        *self.waiting.lock().await = None;
        // `None` is not Linked, or a Link the Hub dropped, which `call` forgot already.
        if self.call(LinkedHub::unlink).await?.is_some() {
            self.forget_kept().await?;
        }
        Ok(())
    }

    /// Makes `call` to the Hub as the Linked Server: `None` while the Server isn't Linked, and
    /// once the Hub answers it doesn't know the Link any more, which forgets the Link. A Hub that
    /// is unreachable or fails is a failure, and keeps the Link.
    pub(crate) async fn call<T, Called>(
        &self,
        call: impl FnOnce(LinkedHub) -> Called,
    ) -> Result<Option<T>, PluginError>
    where
        Called: Future<Output = Result<T, PluginError>>,
    {
        let Some(kept) = self.kept().await? else {
            return Ok(None);
        };
        match call(self.hub.linked(&kept.credential)).await {
            Err(err) if err.is::<LinkDropped>() => {
                self.forget_dropped(&kept).await?;
                Ok(None)
            }
            called => called.map(Some),
        }
    }

    /// Forgets `dropped`, the Link the Hub answered [`LinkDropped`] to, as Unlinking does, so the
    /// Server can Link again. A Link kept since, under another credential, stays.
    async fn forget_dropped(&self, dropped: &Kept) -> Result<(), PluginError> {
        // Held so a poll can't keep a new Link between the check and the delete.
        let _waiting = self.waiting.lock().await;
        if self
            .kept()
            .await?
            .is_some_and(|kept| kept.credential == dropped.credential)
        {
            self.forget_kept().await?;
        }
        Ok(())
    }

    async fn forget_kept(&self) -> Result<(), PluginError> {
        self.kernel.ask::<StoreDelete>(store_key(LINK_KEY)).await?;
        // The Hub Unlinked the Server's Users with it: their codes are gone, and what the Store
        // keeps of their Links names this Link, so none shows once the Server is Linked again.
        self.users
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        *self
            .reachability
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        Ok(())
    }

    /// Tells the Hub `addresses`, unless the Server isn't Linked or the Hub has them already from
    /// this Link and checked them lately. Forgotten on a restart, so the Hub hears them once more
    /// then.
    async fn report(&self, addresses: ServerAddresses) -> Result<(), PluginError> {
        let Some(kept) = self.kept().await? else {
            return Ok(());
        };
        let mut reported = self.reported.lock().await;
        let (link, now) = (kept.id(), Instant::now());
        if reported
            .as_ref()
            .is_some_and(|reported| reported.covers(&link, &addresses, now))
        {
            return Ok(());
        }
        let Some(reachability) = self.call(|hub| hub.set_addresses(&addresses)).await? else {
            return Ok(());
        };
        *self
            .reachability
            .lock()
            .unwrap_or_else(PoisonError::into_inner) =
            reachability.map(|reachability| (link.clone(), reachability));
        *reported = Some(Reported {
            link,
            addresses,
            at: now,
        });
        Ok(())
    }

    /// What the Hub found of the Server's public port through the Link `kept`, if it checked one.
    fn reachability(&self, kept: &Kept) -> Option<Reachability> {
        let found = self
            .reachability
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        found
            .as_ref()
            .filter(|(link, _)| *link == kept.id())
            .map(|(_, reachability)| reachability.clone())
    }

    fn showing(&self, code: &Waiting) -> Link {
        self.waiting_on(code.code.clone())
    }

    /// The Link that waits for someone to enter `code` on the Hub.
    fn waiting_on(&self, code: String) -> Link {
        Link::Waiting {
            code,
            url: self.hub.url().to_owned(),
        }
    }

    /// `token`'s claims, checked against the Hub's key set as last kept, so it passes while the
    /// Hub is unreachable. Only a token naming a key the kept set lacks fetches the set again.
    pub(crate) async fn verify(&self, token: &str) -> Result<Claims, PluginError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let kept = self.stored(KEY_SET_KEY).await?;
        if let Some(key_set) = &kept {
            match token::verify(key_set, token, now) {
                Err(Rejected::UnknownKey) => {}
                verified => return verified.map_err(rejected),
            }
        }
        let key_set = self.hub.key_set().await?;
        self.kernel
            .ask::<StorePut>(StoreEntry {
                key: KEY_SET_KEY.to_owned(),
                value: key_set.to_vec(),
                ..Default::default()
            })
            .await?;
        token::verify(&key_set, token, now).map_err(rejected)
    }

    /// Sends `batch` to the Hub as the Linked Server, or drops it while not Linked.
    async fn share_batch(&self, signal: Signal, batch: Vec<u8>) -> Result<(), PluginError> {
        self.call(|hub| hub.share(signal, batch)).await?;
        Ok(())
    }

    async fn kept(&self) -> Result<Option<Kept>, PluginError> {
        self.stored_json(LINK_KEY).await
    }

    async fn keep(&self, kept: &Kept) -> Result<(), PluginError> {
        self.kernel
            .ask::<StorePut>(StoreEntry {
                key: LINK_KEY.to_owned(),
                value: serde_json::to_vec(kept)?,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    async fn stored(&self, key: &str) -> Result<Option<Vec<u8>>, PluginError> {
        Ok(self.kernel.ask::<StoreGet>(store_key(key)).await?.value)
    }

    async fn stored_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, PluginError> {
        let Some(stored) = self.stored(key).await? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(&stored)?))
    }

    async fn put_json(&self, key: String, value: &impl Serialize) -> Result<(), PluginError> {
        self.kernel
            .ask::<StorePut>(StoreEntry {
                key,
                value: serde_json::to_vec(value)?,
                ..Default::default()
            })
            .await?;
        Ok(())
    }
}

impl ServerLinker for Linker {
    fn link(&self) -> BoxFuture<'_, Result<Link, PluginError>> {
        Box::pin(self.current_link())
    }

    fn start_link<'a>(&'a self, server_name: &'a str) -> BoxFuture<'a, Result<Link, PluginError>> {
        Box::pin(self.start(server_name))
    }

    fn unlink(&self) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(self.forget_link())
    }

    fn user_link(&self, user_id: i64) -> BoxFuture<'_, Result<Option<Link>, PluginError>> {
        Box::pin(self.current_user_link(user_id))
    }

    fn start_user_link<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
    ) -> BoxFuture<'a, Result<Link, PluginError>> {
        Box::pin(self.start_user(user_id, user_name))
    }

    fn unlink_user(&self, user_id: i64) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(self.forget_user_link(user_id))
    }

    fn invite_user<'a>(
        &'a self,
        user_id: i64,
        user_name: &'a str,
        email: &'a str,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(self.invite(user_id, user_name, email))
    }

    fn start_account_sign_in<'a>(
        &'a self,
        return_url: &'a str,
    ) -> BoxFuture<'a, Result<AccountSignIn, PluginError>> {
        Box::pin(self.start_sign_in(return_url))
    }

    fn finish_account_sign_in<'a>(
        &'a self,
        handle: &'a str,
        code: &'a str,
    ) -> BoxFuture<'a, Result<Option<i64>, PluginError>> {
        Box::pin(self.finish_sign_in(handle, code))
    }

    fn set_addresses(&self, addresses: ServerAddresses) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(self.report(addresses))
    }

    fn watch_states(
        &self,
        user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<WatchStateChange>, PluginError>> {
        Box::pin(self.pull(user_id))
    }

    fn track_preferences(
        &self,
        user_id: i64,
    ) -> BoxFuture<'_, Result<Vec<TrackPreference>, PluginError>> {
        Box::pin(self.pull_preferences(user_id))
    }

    fn sync_watchlist(
        &self,
        user_id: i64,
        watchlist: Vec<WatchlistEntry>,
    ) -> BoxFuture<'_, Result<Vec<WatchlistEntry>, PluginError>> {
        Box::pin(self.merge_watchlist(user_id, watchlist))
    }
}

impl DiagnosticsSharer for Linker {
    fn share(&self, signal: Signal, batch: Vec<u8>) -> BoxFuture<'_, Result<(), PluginError>> {
        Box::pin(self.share_batch(signal, batch))
    }
}

fn store_key(key: &str) -> StoreKey {
    StoreKey {
        key: key.to_owned(),
        ..Default::default()
    }
}

fn rejected(rejected: Rejected) -> PluginError {
    match rejected {
        Rejected::UnknownKey => "the Hub's token names a key it doesn't publish".into(),
        Rejected::Invalid(why) => format!("the Hub's token doesn't check out: {why}").into(),
    }
}

#[cfg(test)]
#[path = "linker/linker_tests.rs"]
mod tests;
