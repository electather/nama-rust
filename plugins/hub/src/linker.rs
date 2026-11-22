//! Linking the Server to an Account (ADR 0011): the code it shows while it waits, and the Link it
//! keeps in the Plugin's Store once someone entered it. Its Users' Links are in `users.rs`.

mod users;

use std::{
    collections::HashMap,
    sync::{Arc, PoisonError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nama_plugin_api::{
    BoxFuture, PluginError,
    automation::{
        Kernel,
        functions::{StoreDelete, StoreGet, StorePut},
    },
    contract::{StoreEntry, StoreKey},
    server_link::{Link, ServerLinker},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    hub::{Hub, Polling},
    token::{self, Claims, Rejected},
};

/// The Store key of the Link, a [`Kept`] as JSON.
const LINK_KEY: &str = "link";

/// The Store key of the Hub's key set, as the Hub publishes it.
const KEY_SET_KEY: &str = "key-set";

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
}

impl Linker {
    pub(crate) fn new(hub: Hub, kernel: Arc<dyn Kernel>) -> Self {
        Self {
            hub,
            kernel,
            waiting: Mutex::default(),
            users: std::sync::Mutex::default(),
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
        let Some(kept) = self.kept().await? else {
            return Ok(());
        };
        self.hub.unlink(&kept.credential).await?;
        self.kernel.ask::<StoreDelete>(store_key(LINK_KEY)).await?;
        // The Hub Unlinked the Server's Users with it: their codes are gone, and once Linked again
        // the Server asks the Hub before showing a User as Linked.
        self.users
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        Ok(())
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

    async fn kept(&self) -> Result<Option<Kept>, PluginError> {
        let Some(kept) = self.stored(LINK_KEY).await? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(&kept)?))
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
