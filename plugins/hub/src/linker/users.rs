//! Linking the Server's Users to Accounts (ADR 0008, ADR 0011): the code a User shows while it
//! waits, and the Account each is Linked to, kept in the Plugin's Store so Settings still shows it
//! while the Hub is unreachable (ADR 0002).

use std::{
    sync::PoisonError,
    time::{Duration, Instant},
};

use nama_plugin_api::{
    PluginError,
    automation::functions::{StoreDelete, StorePut},
    contract::StoreEntry,
    server_link::Link,
};

use super::{Linker, store_key};

/// How long what the Hub last said of a User's Link stands before Settings asks it again, so
/// opening Settings doesn't call the Hub every time (ADR 0094). While a code waits, it asks every
/// time, which is how the Plugin learns the code was entered.
const ASK_AGAIN_AFTER: Duration = Duration::from_secs(60);

/// What the Plugin knows of one User's Link beyond what it keeps in the Store.
#[derive(Default)]
pub(super) struct UserState {
    /// The code they show, and when it stops working.
    code: Option<(String, Instant)>,
    /// When the Hub last said whether they are Linked.
    asked: Option<Instant>,
}

impl Linker {
    /// Where Linking `user_id` stands, or `None` while the Server isn't Linked.
    pub(super) async fn current_user_link(
        &self,
        user_id: i64,
    ) -> Result<Option<Link>, PluginError> {
        let Some(kept) = self.kept().await? else {
            return Ok(None);
        };
        if self.asks_hub(user_id) {
            // Unreachable, the Hub says nothing, and what it said last stands.
            if let Ok(account) = self.hub.user_account(&kept.credential, user_id).await {
                self.remember_account(user_id, &account).await?;
            }
        }
        let account = self.stored(&user_key(user_id)).await?;
        Ok(Some(match account {
            Some(account) => Link::Linked {
                account: String::from_utf8(account)?,
            },
            None => self
                .user_code(user_id)
                .map_or(Link::NotLinked, |code| self.waiting_on(code)),
        }))
    }

    /// Starts Linking `user_id`, whom the Hub lists as `name`, and answers the code they show.
    pub(super) async fn start_user(&self, user_id: i64, name: &str) -> Result<Link, PluginError> {
        let kept = self
            .kept()
            .await?
            .ok_or("the Server isn't Linked to the Hub, so its Users can't Link")?;
        let started = self.hub.start_user(&kept.credential, user_id, name).await?;
        let lasts = Duration::from_secs(started.expires_in_seconds.try_into().unwrap_or(0));
        let code = (started.user_code.clone(), Instant::now() + lasts);
        self.with_user(user_id, |user| user.code = Some(code));
        Ok(self.waiting_on(started.user_code))
    }

    /// Unlinks `user_id` on the Hub, then here, and forgets the code they show.
    pub(super) async fn forget_user_link(&self, user_id: i64) -> Result<(), PluginError> {
        if let Some(kept) = self.kept().await? {
            self.hub.unlink_user(&kept.credential, user_id).await?;
        }
        self.with_user(user_id, |user| user.code = None);
        self.remember_account(user_id, "").await
    }

    /// Keeps `account`, the email of the Account the Hub says `user_id` is Linked to, or forgets
    /// their Link if it is empty.
    async fn remember_account(&self, user_id: i64, account: &str) -> Result<(), PluginError> {
        let key = user_key(user_id);
        if account.is_empty() {
            self.kernel.ask::<StoreDelete>(store_key(&key)).await?;
        } else {
            self.kernel
                .ask::<StorePut>(StoreEntry {
                    key,
                    value: account.as_bytes().to_vec(),
                    ..Default::default()
                })
                .await?;
        }
        let linked = !account.is_empty();
        self.with_user(user_id, |user| {
            user.asked = Some(Instant::now());
            if linked {
                user.code = None;
            }
        });
        Ok(())
    }

    /// Whether to ask the Hub about `user_id`'s Link: while their code waits, or once what it
    /// said last is [`ASK_AGAIN_AFTER`] old.
    fn asks_hub(&self, user_id: i64) -> bool {
        let waiting = self.user_code(user_id).is_some();
        self.with_user(user_id, |user| {
            waiting
                || user
                    .asked
                    .is_none_or(|asked| asked.elapsed() >= ASK_AGAIN_AFTER)
        })
    }

    /// The code `user_id` shows, unless it expired.
    fn user_code(&self, user_id: i64) -> Option<String> {
        self.with_user(user_id, |user| {
            user.code.take_if(|(_, expires)| *expires <= Instant::now());
            user.code.as_ref().map(|(code, _)| code.clone())
        })
    }

    fn with_user<R>(&self, user_id: i64, change: impl FnOnce(&mut UserState) -> R) -> R {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        change(users.entry(user_id).or_default())
    }
}

/// The Store key of the email of the Account `user_id` is Linked to.
fn user_key(user_id: i64) -> String {
    format!("user-link/{user_id}")
}
