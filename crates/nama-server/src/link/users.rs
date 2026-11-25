//! Linking the Server's Users to Accounts (ADR 0008, ADR 0011): the code a User shows while it
//! waits, and the Account each is Linked to, kept in `hub_user_links` so Settings still shows it
//! while the Hub is unreachable (ADR 0002). Signing in to the Server with an Account, and Users an
//! Admin creates from one (ADR 0092), too: the Hub runs the sign-in as a device flow, the person
//! approves it on the Hub's page, and the Server collects the token naming their User with its
//! credential, so the token never passes through the browser.

use std::{
    collections::HashMap,
    sync::PoisonError,
    time::{Duration, Instant},
};

use nama_plugin_api::server_link::{AccountSignIn, Link};

use super::{
    hub::Failure,
    server::{Kept, NOT_LINKED, ServerLink},
};
use crate::db;

/// How long after the Hub was last asked of a User's Link it is asked again, so opening Settings
/// or changing Watch State doesn't call the Hub every time. While a code waits, it asks every
/// time, which is how the Server learns the code was entered.
const ASK_AGAIN_AFTER: Duration = Duration::from_mins(1);

/// What is known of the Server's Users' Links beyond what `hub_user_links` keeps.
#[derive(Default)]
pub(super) struct KnownUsers {
    /// The [`Kept::id`] of the Server's Link it was learned through.
    server_link: Option<String>,
    by_id: HashMap<i64, UserState>,
}

/// What is known of one User's Link beyond what `hub_user_links` keeps.
#[derive(Default)]
struct UserState {
    /// The code they show, and when it stops working.
    code: Option<(String, Instant)>,
    /// When the Hub was last asked whether they are Linked.
    asked: Option<Instant>,
}

impl ServerLink {
    /// Where Linking `user_id` stands, or `None` while the Server isn't Linked, such as once the
    /// Hub answered it doesn't know the Server's Link any more.
    pub(super) async fn user_link(&self, user_id: i64) -> Result<Option<Link>, Failure> {
        let Some(kept) = self.kept_for_users().await? else {
            return Ok(None);
        };
        if self.asks_hub(user_id) {
            match self.call(|hub| hub.user_account(user_id)).await {
                Ok(Some(account)) => self.remember_account(user_id, &kept, &account).await?,
                Ok(None) => return Ok(None),
                // What it said last stands, and it isn't asked again until it's due, so an outage
                // doesn't make every Watch State change call it.
                Err(_) => self.with_user(user_id, |user| user.asked = Some(db::instant_now())),
            }
        }
        Ok(Some(match self.kept_account(user_id, &kept).await? {
            Some(account) => Link::Linked {
                account,
                reachability: None,
            },
            None => self
                .user_code(user_id)
                .map_or(Link::NotLinked, |code| self.waiting_on(code)),
        }))
    }

    /// Starts Linking `user_id`, whom the Hub lists as `name`, and answers the code they show, or
    /// their Link if they are Linked already.
    pub(super) async fn start_user(&self, user_id: i64, name: &str) -> Result<Link, Failure> {
        let kept = self.kept_for_users().await?.ok_or(NOT_LINKED)?;
        let started = self.call(|hub| hub.start_user(user_id, name)).await?;
        let Some(started) = started.ok_or(NOT_LINKED)? else {
            // Linked since the Hub was last asked, so Settings shows that Link instead.
            let account = self.call(|hub| hub.user_account(user_id)).await?;
            let account = account.ok_or(NOT_LINKED)?;
            self.remember_account(user_id, &kept, &account).await?;
            return Ok(if account.is_empty() {
                Link::NotLinked
            } else {
                Link::Linked {
                    account,
                    reachability: None,
                }
            });
        };
        let lasts = Duration::from_secs(started.expires_in_seconds.try_into().unwrap_or(0));
        let code = (started.user_code.clone(), db::instant_now() + lasts);
        self.with_user(user_id, |user| user.code = Some(code));
        Ok(self.waiting_on(started.user_code))
    }

    /// Unlinks `user_id` on the Hub, then here, and forgets the code they show.
    pub(super) async fn unlink_user(&self, user_id: i64) -> Result<(), Failure> {
        self.with_user(user_id, |user| user.code = None);
        // Without the Server's Link, the User's kept one is from an earlier Link, and never shows.
        let Some(kept) = self.kept_for_users().await? else {
            return Ok(());
        };
        self.call(|hub| hub.unlink_user(user_id)).await?;
        self.remember_account(user_id, &kept, "").await
    }

    /// Asks the Account with `email`, if any, to accept `user_id`, named `name`.
    pub(super) async fn invite(
        &self,
        user_id: i64,
        name: &str,
        email: &str,
    ) -> Result<(), Failure> {
        let invited = self.call(|hub| hub.invite_user(user_id, name, email));
        Ok(invited.await?.ok_or(NOT_LINKED)?)
    }

    /// Starts a sign-in with an Account, which the Hub's page sends back to `return_url`.
    pub(super) async fn start_sign_in(&self, return_url: &str) -> Result<AccountSignIn, Failure> {
        let started = self.call(|hub| hub.start_sign_in(return_url)).await?;
        let started = started.ok_or(NOT_LINKED)?;
        Ok(AccountSignIn {
            url: format!("{}/server-sign-in/{}", self.pages_url(), started.id),
            handle: started.device_code,
        })
    }

    /// The User the sign-in `handle` signs in as, once approved and finished with the one-time
    /// `code` the Hub sent the person back with, as the Hub's signed token names them.
    pub(super) async fn finish_sign_in(
        &self,
        handle: &str,
        code: &str,
    ) -> Result<Option<i64>, Failure> {
        let polled = self.call(|hub| hub.poll_sign_in(handle, code)).await?;
        let Some(token) = polled.ok_or(NOT_LINKED)? else {
            return Ok(None);
        };
        let user = self.sign_in_subject(&token).await?;
        Ok(Some(user.parse().map_err(|_not_a_number| {
            format!("the Hub's token names no User: {user:?}")
        })?))
    }

    /// The Server's Link, forgetting what is known of its Users' Links when it isn't the one that
    /// was learned through: the Hub Unlinked them with an earlier Link.
    async fn kept_for_users(&self) -> Result<Option<Kept>, Failure> {
        let kept = self.kept().await?;
        let id = kept.as_ref().map(Kept::id);
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        if users.server_link != id {
            *users = KnownUsers {
                server_link: id,
                by_id: HashMap::default(),
            };
        }
        drop(users);
        Ok(kept)
    }

    /// The email of the Account `user_id` Linked to under the Server's Link `kept`, as last kept.
    async fn kept_account(&self, user_id: i64, kept: &Kept) -> Result<Option<String>, Failure> {
        let server_link = kept.id();
        Ok(sqlx::query_scalar!(
            "SELECT account FROM hub_user_links WHERE user_id = ? AND server_link = ?",
            user_id,
            server_link
        )
        .fetch_optional(self.db.reader())
        .await?)
    }

    /// Keeps `account`, the email of the Account the Hub says `user_id` is Linked to under the
    /// Server's Link `kept`, or forgets their Link if it is empty.
    async fn remember_account(
        &self,
        user_id: i64,
        kept: &Kept,
        account: &str,
    ) -> Result<(), Failure> {
        let linked = !account.is_empty();
        if linked {
            self.keep_account(user_id, kept.id(), account.to_owned())
                .await?;
        } else {
            self.forget_account(user_id).await?;
        }
        self.with_user(user_id, |user| {
            user.asked = Some(db::instant_now());
            if linked {
                user.code = None;
            }
        });
        Ok(())
    }

    async fn keep_account(
        &self,
        user_id: i64,
        server_link: String,
        account: String,
    ) -> Result<(), sqlx::Error> {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT OR REPLACE INTO hub_user_links (user_id, server_link, account)
                         VALUES (?, ?, ?)",
                        user_id,
                        server_link,
                        account
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    async fn forget_account(&self, user_id: i64) -> Result<(), sqlx::Error> {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!("DELETE FROM hub_user_links WHERE user_id = ?", user_id)
                        .execute(&mut **tx)
                        .await?;
                    Ok(())
                })
            })
            .await
    }

    /// Whether to ask the Hub about `user_id`'s Link: while their code waits, or once it was last
    /// asked [`ASK_AGAIN_AFTER`] ago.
    fn asks_hub(&self, user_id: i64) -> bool {
        let waiting = self.user_code(user_id).is_some();
        self.with_user(user_id, |user| {
            waiting
                || user
                    .asked
                    .is_none_or(|asked| db::instant_now().duration_since(asked) >= ASK_AGAIN_AFTER)
        })
    }

    /// The code `user_id` shows, unless it expired.
    fn user_code(&self, user_id: i64) -> Option<String> {
        self.with_user(user_id, |user| {
            user.code
                .take_if(|(_, expires)| *expires <= db::instant_now());
            user.code.as_ref().map(|(code, _)| code.clone())
        })
    }

    fn with_user<R>(&self, user_id: i64, change: impl FnOnce(&mut UserState) -> R) -> R {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        change(users.by_id.entry(user_id).or_default())
    }
}
