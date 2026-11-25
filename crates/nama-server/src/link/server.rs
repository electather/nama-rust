//! The Server's Link with the Hub, which the Kernel keeps (ADR 0011, ADR 0105): the code the Server
//! shows while it waits, the credential it keeps in `hub_link` once someone entered it, the Server
//! Addresses it tells the Hub, and the Hub's key set in `hub_key_set`, which the tokens the Hub
//! signs are checked against. [`ServerLink`] alone reads and writes both tables, and every Hub call
//! made as the Server goes through [`ServerLink::call`], so one rule decides what a Link the Hub
//! dropped means. Its Users' Links and signing in with an Account are in `users.rs`, and through it go
//! Linked Users' changes waiting for the Hub, and their pulls and Watchlist syncs (`sync.rs`).

use std::{
    collections::{HashMap, hash_map::Entry},
    future::Future,
    sync::{Arc, PoisonError},
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use bytes::Bytes;
use connectrpc::{ConnectError, ErrorCode};
use nama_plugin_api::{
    BoxFuture, PluginError,
    points::{TrackPreference, WatchStateChange},
    server_link::{Link, Reachability, ServerAddresses, WatchlistEntry},
};
use nama_plugin_hub::KernelLink;
use ring::digest;
use serde::de::DeserializeOwned;
use sqlx::SqliteConnection;
use tokio::sync::{Mutex, watch};

use super::{
    Linking,
    hub::{Failure, Hub, LinkDropped, LinkedHub, Polling},
    sync::{self, Kind, Queued},
    token::{self, Claims, Rejected},
    users::KnownUsers,
};
use crate::{
    db::{self, Database},
    diagnostics::Signal,
};

/// How long the Hub's check of the Server's public port stands: the same Server Addresses go to
/// the Hub again after this, so it checks again, such as after the router's public IP changed.
const RECHECK_EVERY: Duration = Duration::from_mins(30);

/// Who a Link whose token couldn't be checked belongs to, as Settings shows it.
const UNCONFIRMED_ACCOUNT: &str = "an Account the Hub didn't confirm";

/// Why a call only a Linked Server makes wasn't made.
pub(super) const NOT_LINKED: &str = "the Server isn't Linked to the Hub";

/// A code the Server shows, waiting for someone to enter it on the Hub.
struct Waiting {
    device_code: String,
    code: String,
    poll_every: Duration,
    next_poll: Instant,
    expires: Instant,
}

/// The Link as `hub_link` keeps it.
pub(super) struct Kept {
    /// What the Server presents on every later Hub call (ADR 0098).
    credential: String,
    /// The Account's email, as the Hub's signed token named it; `None` if the token couldn't be
    /// checked when the credential came.
    account: Option<String>,
    /// The Server's id at the Hub; `None` until a token the Hub signed named it.
    server_id: Option<String>,
}

impl Kept {
    /// Names this Link apart from the Server's earlier ones, without holding its credential.
    pub(super) fn id(&self) -> String {
        URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, self.credential.as_bytes()))
    }
}

/// The Server Addresses the Hub has.
struct Reported {
    /// The credential of the Link it got them through.
    credential: String,
    addresses: ServerAddresses,
    /// When the Hub got them, and checked their public port.
    at: Instant,
}

impl Reported {
    /// Whether the Hub needs no report of `addresses` through the Link holding `credential` at
    /// `now`: it has them, and checked their public port less than [`RECHECK_EVERY`] ago.
    fn covers(&self, credential: &str, addresses: &ServerAddresses, now: Instant) -> bool {
        self.credential == credential
            && self.addresses == *addresses
            && now.duration_since(self.at) < RECHECK_EVERY
    }
}

/// The Server's Link with the Hub at one address.
pub(crate) struct ServerLink {
    hub: Hub,
    pub(super) db: Arc<Database>,
    /// Held across a poll, so two polls never race for the one credential.
    waiting: Mutex<Option<Waiting>>,
    /// Held across telling the Hub, so a report never overtakes another.
    reported: Mutex<Option<Reported>>,
    /// What the Hub found of the Server's public port and the names it gave it, with the
    /// credential of the Link it found them through. Apart from `reported`, so showing the Link
    /// never waits on a report.
    reachability: std::sync::Mutex<Option<(String, Reachability)>>,
    /// Told each time the Hub answers a report.
    found: watch::Sender<()>,
    /// Held while the changes waiting are sent, so none goes twice.
    sending: Mutex<()>,
    /// What is known of each User's Link beyond what `hub_user_links` keeps.
    pub(super) users: std::sync::Mutex<KnownUsers>,
}

impl std::fmt::Debug for ServerLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerLink")
            .field("hub", &self.hub)
            .finish_non_exhaustive()
    }
}

impl ServerLink {
    /// The Link with the Hub at `url`, kept in `db`.
    pub(crate) fn new(url: &str, db: Arc<Database>) -> Result<Self, Failure> {
        Ok(Self {
            hub: Hub::new(url)?,
            db,
            waiting: Mutex::default(),
            reported: Mutex::default(),
            reachability: std::sync::Mutex::default(),
            found: watch::channel(()).0,
            sending: Mutex::default(),
            users: std::sync::Mutex::default(),
        })
    }

    /// Wakes each time the Hub answers a report of the Server Addresses.
    pub(super) fn follow_found(&self) -> watch::Receiver<()> {
        self.found.subscribe()
    }

    /// Where the Link stands. While a code waits, this is also when the Hub is asked whether it
    /// was entered.
    pub(super) async fn link(&self) -> Result<Link, Failure> {
        let mut waiting = self.waiting.lock().await;
        if let Some(code) = waiting.as_mut() {
            match self.poll(code).await? {
                Some(showing) => return Ok(showing),
                None => *waiting = None,
            }
        }
        Ok(match self.kept().await? {
            Some(kept) => Link::Linked {
                reachability: self.reachability(&kept.credential),
                account: kept
                    .account
                    .unwrap_or_else(|| UNCONFIRMED_ACCOUNT.to_owned()),
            },
            None => Link::NotLinked,
        })
    }

    /// Whether `origin` is the Hub's and the Server is Linked with it, reading only `hub_link`.
    pub(super) async fn is_linked_hub_origin(&self, origin: &str) -> bool {
        self.hub.origin() == Some(origin)
            && (self.kept().await)
                .inspect_err(|err| tracing::error!("couldn't read the Server's Link: {err}"))
                .is_ok_and(|kept| kept.is_some())
    }

    /// Asks the Hub whether `code` was entered, once its poll interval has passed. `Some` while
    /// it still waits, `None` once it is gone: expired, or entered and its Link kept.
    async fn poll(&self, code: &mut Waiting) -> Result<Option<Link>, Failure> {
        let now = db::instant_now();
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
        self.keep(&polled.credential, None, None).await?;
        let claims = self.verify(&polled.token).await?;
        (self.keep(&polled.credential, Some(claims.email), Some(claims.sub))).await?;
        Ok(None)
    }

    /// Starts Linking the Server, which the Hub lists as `server_name`, and answers the code.
    pub(super) async fn start(&self, server_name: &str) -> Result<Link, Failure> {
        // Held from the check on: a poll keeping a credential meanwhile would otherwise see its
        // Link overwritten by the next, leaving one on the Hub that nothing could Unlink.
        let mut waiting = self.waiting.lock().await;
        if self.kept().await?.is_some() {
            return Err("the Server is Linked already: Unlink it first".into());
        }
        let started = self.hub.start(server_name).await?;
        let now = db::instant_now();
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

    /// Unlinks the Server, telling the Hub, or forgets its waiting code.
    pub(super) async fn unlink(&self) -> Result<(), Failure> {
        *self.waiting.lock().await = None;
        let Some(kept) = self.kept().await? else {
            return Ok(());
        };
        match self.hub.linked(&kept.credential).unlink().await {
            Err(err) if !err.is::<LinkDropped>() => Err(err),
            _unlinked => Ok(self.forget_link(&kept.credential).await?),
        }
    }

    /// Makes `call` to the Hub as the Linked Server: `None` while the Server isn't Linked, and
    /// once the Hub answers it doesn't know the Link any more, which forgets the Link. A Hub that
    /// is unreachable or fails is a failure, and keeps the Link.
    pub(super) async fn call<T, Called>(
        &self,
        call: impl FnOnce(LinkedHub) -> Called,
    ) -> Result<Option<T>, Failure>
    where
        Called: Future<Output = Result<T, Failure>>,
    {
        let Some(kept) = self.kept().await? else {
            return Ok(None);
        };
        match call(self.hub.linked(&kept.credential)).await {
            Err(err) if err.is::<LinkDropped>() => {
                self.forget_link(&kept.credential).await?;
                Ok(None)
            }
            called => called.map(Some),
        }
    }

    /// Tells the Hub `addresses`, unless the Server isn't Linked or the Hub has them already from
    /// this Link and checked them lately. Forgotten on a restart, so the Hub hears them once more
    /// then.
    pub(super) async fn report(&self, addresses: ServerAddresses) -> Result<(), Failure> {
        let Some(kept) = self.kept().await? else {
            return Ok(());
        };
        let mut reported = self.reported.lock().await;
        let now = db::instant_now();
        if reported
            .as_ref()
            .is_some_and(|reported| reported.covers(&kept.credential, &addresses, now))
        {
            return Ok(());
        }
        let told = addresses.clone();
        let Some(reachability) = self.call(|hub| hub.set_addresses(told)).await? else {
            return Ok(());
        };
        *self
            .reachability
            .lock()
            .unwrap_or_else(PoisonError::into_inner) =
            reachability.map(|reachability| (kept.credential.clone(), reachability));
        self.found.send_replace(());
        *reported = Some(Reported {
            credential: kept.credential,
            addresses,
            at: now,
        });
        Ok(())
    }

    /// Has the Hub publish `values` as the DNS-01 challenge for a certificate for the Server's
    /// names, or remove it when empty (ADR 0057).
    pub(super) async fn set_certificate_challenge(
        &self,
        values: Vec<String>,
    ) -> Result<(), Failure> {
        let set = self.call(|hub| hub.set_certificate_challenge(values));
        Ok(set.await?.ok_or(NOT_LINKED)?)
    }

    /// A token the Hub signed that proves to a Relay the Server holds its own name (ADR 0057).
    pub(super) async fn relay_token(&self) -> Result<String, Failure> {
        let token = self.call(LinkedHub::relay_token);
        Ok(token.await?.ok_or(NOT_LINKED)?)
    }

    /// Sends the changes and Viewing Events waiting for the Hub, a page per call, oldest first, each
    /// kind apart, so one the Hub refuses holds back only its own. Those of a User `linking` says
    /// isn't Linked are forgotten unsent, as Linking pushes their whole Watch State and Preferences
    /// anyway, and their plays are never the Hub's. A page the Hub refuses as invalid is dropped,
    /// as it would refuse it again, and without the Server's Link everything is, having nowhere to
    /// go. Any other failure, such as the Hub being down, keeps the page and the rest.
    pub(super) async fn send_queued(&self, linking: &Linking) -> Result<(), Failure> {
        let _sending = self.sending.lock().await;
        let watch_states = self.send_waiting(Kind::WatchState, linking).await;
        let preferences = self.send_waiting(Kind::Preference, linking).await;
        let viewing_events = self.send_waiting(Kind::ViewingEvent, linking).await;
        watch_states.and(preferences).and(viewing_events)
    }

    /// [`ServerLink::send_queued`] of what waits of `kind`.
    async fn send_waiting(&self, kind: Kind, linking: &Linking) -> Result<(), Failure> {
        let mut linked = HashMap::new();
        loop {
            let page = sync::waiting(&self.db, kind).await?;
            let Some(last) = page.last().map(|queued| queued.id) else {
                return Ok(());
            };
            let to_send = of_linked_users(page, linking, &mut linked).await?;
            let sent = if to_send.is_empty() {
                Ok(Some(()))
            } else {
                self.push(kind, &to_send).await
            };
            match sent {
                Ok(Some(())) => {}
                Ok(None) => return Ok(sync::forget_all(&self.db).await?),
                Err(err) if refused_as_invalid(&err) => {
                    tracing::warn!(
                        ?kind,
                        "the Hub refused a page of changes as invalid, so it was dropped: {err}"
                    );
                }
                Err(err) => return Err(err),
            }
            sync::forget_through(&self.db, kind, last).await?;
        }
    }

    /// Sends `page`, of `kind`, in the one Hub call that takes it, as [`ServerLink::call`] answers.
    async fn push(&self, kind: Kind, page: &[Queued]) -> Result<Option<()>, Failure> {
        match kind {
            Kind::WatchState => (self.call(|hub| hub.push_watch_states(parsed(page)))).await,
            Kind::Preference => (self.call(|hub| hub.push_track_preferences(parsed(page)))).await,
            Kind::ViewingEvent => (self.call(|hub| hub.record_viewing_events(parsed(page)))).await,
        }
    }

    /// The Watch State of the Account the User `user_id` is Linked to; empty while the Server
    /// isn't Linked.
    pub(super) async fn pull_watch_states(
        &self,
        user_id: i64,
    ) -> Result<Vec<WatchStateChange>, Failure> {
        let pulled = self.call(|hub| hub.pull_watch_states(user_id)).await?;
        Ok((pulled.unwrap_or_default().into_iter())
            .map(sync::watch_state_from_proto)
            .collect())
    }

    /// The Track Preferences of the Account the User `user_id` is Linked to; empty while the
    /// Server isn't Linked.
    pub(super) async fn pull_track_preferences(
        &self,
        user_id: i64,
    ) -> Result<Vec<TrackPreference>, Failure> {
        let pulled = self.call(|hub| hub.pull_track_preferences(user_id)).await?;
        Ok((pulled.unwrap_or_default().into_iter())
            .filter_map(sync::preference_from_proto)
            .collect())
    }

    /// Merges `watchlist`, the User `user_id`'s, into the Account they are Linked to, and answers
    /// the Account's; empty while the Server isn't Linked.
    pub(super) async fn sync_watchlist(
        &self,
        user_id: i64,
        watchlist: Vec<WatchlistEntry>,
    ) -> Result<Vec<WatchlistEntry>, Failure> {
        let entries = sync::watchlist_to_proto(watchlist);
        let synced = self
            .call(|hub| hub.sync_watchlist(user_id, entries))
            .await?;
        Ok((synced.unwrap_or_default().into_iter())
            .filter_map(sync::entry_from_proto)
            .collect())
    }

    /// Sends `batch`, an OTLP/HTTP protobuf export request of `signal`, to the Hub for Nama's
    /// developers, or drops it while the Server isn't Linked.
    pub(super) async fn share(&self, signal: Signal, batch: Bytes) -> Result<(), Failure> {
        self.call(|hub| hub.share(signal, batch)).await?;
        Ok(())
    }

    /// Forgets the Link holding `credential`, as Unlinking does, so the Server can Link again. A
    /// Link kept since, under another credential, stays.
    async fn forget_link(&self, credential: &str) -> Result<(), sqlx::Error> {
        let credential = credential.to_owned();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!("DELETE FROM hub_link WHERE credential = ?", credential)
                        .execute(&mut **tx)
                        .await?;
                    Ok(())
                })
            })
            .await
    }

    /// What the Hub found of the Server's public port and names through the Link holding
    /// `credential`, if it answered a report.
    fn reachability(&self, credential: &str) -> Option<Reachability> {
        let found = self
            .reachability
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        found
            .as_ref()
            .filter(|(link, _)| link == credential)
            .map(|(_, reachability)| reachability.clone())
    }

    fn showing(&self, code: &Waiting) -> Link {
        self.waiting_on(code.code.clone())
    }

    /// The Link that waits for someone to enter `code` on the Hub.
    pub(super) fn waiting_on(&self, code: String) -> Link {
        Link::Waiting {
            code,
            url: self.hub.url().to_owned(),
        }
    }

    /// The address of the Hub's pages.
    pub(super) fn pages_url(&self) -> &str {
        self.hub.url()
    }

    /// `token`'s claims, checked as [`ServerLink::checked`] does, failing when it doesn't pass.
    pub(super) async fn verify(&self, token: &str) -> Result<Claims, Failure> {
        self.checked(token).await?.map_err(rejected)
    }

    /// `token`'s claims, or why it didn't pass, checked against the Hub's key set as last kept, so
    /// it passes while the Hub is unreachable. Only a token naming a key the kept set lacks fetches
    /// the set again, which fails while the Hub is unreachable.
    async fn checked(&self, token: &str) -> Result<Result<Claims, Rejected>, Failure> {
        let now = u64::try_from(db::now()).unwrap_or_default();
        let kept = sqlx::query_scalar!("SELECT key_set FROM hub_key_set")
            .fetch_optional(self.db.reader())
            .await?;
        if let Some(key_set) = &kept {
            match token::verify(key_set, token, now) {
                Err(Rejected::UnknownKey) => {}
                verified => return Ok(verified),
            }
        }
        let key_set = self.hub.key_set().await?.to_vec();
        let keeping = key_set.clone();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT OR REPLACE INTO hub_key_set (id, key_set) VALUES (1, ?)",
                        keeping
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok::<_, sqlx::Error>(())
                })
            })
            .await?;
        Ok(token::verify(&key_set, token, now))
    }

    /// Whom `token`, a sign-in token the Hub signed, names, once its audience is this Server. A
    /// Link the Hub Plugin made doesn't know the Server's id: it takes the audience of the first
    /// such token, which the Hub hands only to the Server it is for, over its Link.
    pub(super) async fn sign_in_subject(&self, token: &str) -> Result<String, Failure> {
        let claims = self.verify(token).await?;
        let kept = self.kept().await?.ok_or(NOT_LINKED)?;
        if learns_audience(kept.server_id.as_deref(), &claims.aud)? {
            self.learn_server_id(kept.credential, claims.aud).await?;
        }
        Ok(claims.sub)
    }

    /// The id of the User `token` names, a token the Hub signed for the web app at the Hub to trade
    /// for a Device (ADR 0105), using it up. `None`, logged, when it is for another Server, or
    /// isn't one the Hub signed, unexpired and unused, since the page's caller chose it.
    pub(super) async fn hub_token_user(&self, token: &str) -> Result<Option<i64>, Failure> {
        let claims = match self.checked(token).await? {
            Ok(claims) => claims,
            Err(why) => {
                tracing::info!("{}", rejected(why));
                return Ok(None);
            }
        };
        let own = self.kept().await?.and_then(|kept| kept.server_id);
        let passes_until = claims.passes_until();
        let user = claims.sub.parse().ok();
        let (Some(jti), Some(user), true) = (claims.jti, user, own == Some(claims.aud)) else {
            tracing::info!("a Hub token was refused: it isn't for a Device here");
            return Ok(None);
        };
        Ok(self.use_up(jti, passes_until).await?.then_some(user))
    }

    /// Whether the token `jti` passing until `passes_until` was unused, keeping it as used, and
    /// forgetting those that no longer pass.
    async fn use_up(&self, jti: String, passes_until: u64) -> Result<bool, sqlx::Error> {
        let now = db::now();
        let passes_until = i64::try_from(passes_until).unwrap_or(i64::MAX);
        // `checked` read the clock earlier: a token that stopped passing since would find its
        // first use forgotten by the cleanup below, and pass a second time.
        if passes_until < now {
            return Ok(false);
        }
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!("DELETE FROM hub_tokens_used WHERE expires_at < ?", now)
                        .execute(&mut **tx)
                        .await?;
                    let kept = sqlx::query!(
                        "INSERT OR IGNORE INTO hub_tokens_used (jti, expires_at) VALUES (?, ?)",
                        jti,
                        passes_until
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(kept.rows_affected() == 1)
                })
            })
            .await
    }

    /// Keeps `server_id` as the Server's id at the Hub, for the Link holding `credential`.
    async fn learn_server_id(&self, credential: String, server_id: String) -> Result<(), Failure> {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE hub_link SET server_id = ? WHERE credential = ? AND server_id IS NULL",
                        server_id,
                        credential
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok::<_, sqlx::Error>(())
                })
            })
            .await?;
        Ok(())
    }

    pub(super) async fn kept(&self) -> Result<Option<Kept>, sqlx::Error> {
        let kept = sqlx::query!("SELECT credential, account, server_id FROM hub_link")
            .fetch_optional(self.db.reader())
            .await?;
        Ok(kept.map(|kept| Kept {
            credential: kept.credential,
            account: kept.account,
            server_id: kept.server_id,
        }))
    }

    async fn keep(
        &self,
        credential: &str,
        account: Option<String>,
        server_id: Option<String>,
    ) -> Result<(), sqlx::Error> {
        let credential = credential.to_owned();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT OR REPLACE INTO hub_link (id, credential, account, server_id)
                         VALUES (1, ?, ?, ?)",
                        credential,
                        account,
                        server_id
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok(())
                })
            })
            .await
    }
}

/// Whether the Server is Linked, as `conn` reads it.
pub(super) async fn is_linked(conn: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
    let linked =
        sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM hub_link) AS "linked!: bool""#)
            .fetch_one(conn)
            .await?;
    Ok(linked)
}

/// The changes of `page` whose User `linking` says is Linked, asking of each User once, as
/// `linked` remembers.
async fn of_linked_users(
    page: Vec<Queued>,
    linking: &Linking,
    linked: &mut HashMap<i64, bool>,
) -> Result<Vec<Queued>, super::Error> {
    for queued in &page {
        if let Entry::Vacant(unasked) = linked.entry(queued.user_id) {
            unasked.insert(linking.is_user_linked(queued.user_id).await?);
        }
    }
    Ok((page.into_iter())
        .filter(|queued| linked.get(&queued.user_id) == Some(&true))
        .collect())
}

/// The Hub API's messages `queued` holds, leaving out, logged, any that doesn't read as one.
fn parsed<T: DeserializeOwned>(queued: &[Queued]) -> Vec<T> {
    (queued.iter())
        .filter_map(|queued| {
            serde_json::from_str(&queued.change)
                .inspect_err(|err| {
                    tracing::warn!(
                        user = queued.user_id,
                        "a change waiting for the Hub doesn't read: {err}"
                    );
                })
                .ok()
        })
        .collect()
}

/// Whether the Hub refused a call as invalid, which it would refuse again.
fn refused_as_invalid(err: &Failure) -> bool {
    err.downcast_ref::<ConnectError>()
        .is_some_and(|refused| refused.code == ErrorCode::InvalidArgument)
}

/// What the Hub Plugin calls the Hub with until the rest of Linking moves into the Kernel (#2203).
impl KernelLink for ServerLink {
    fn hub_url(&self) -> &str {
        self.hub.url()
    }

    fn credential(&self) -> BoxFuture<'_, Result<Option<String>, PluginError>> {
        Box::pin(async { Ok(self.kept().await?.map(|kept| kept.credential)) })
    }

    fn forget<'a>(&'a self, credential: &'a str) -> BoxFuture<'a, Result<(), PluginError>> {
        Box::pin(async { Ok(self.forget_link(credential).await?) })
    }
}

/// Whether a sign-in token for `audience` is for this Server, whose id at the Hub is `own` once
/// known: `true` when `audience` is to be kept as its id.
fn learns_audience(own: Option<&str>, audience: &str) -> Result<bool, &'static str> {
    match own {
        Some(own) if own == audience => Ok(false),
        Some(_) => Err("the Hub's token is for another Server"),
        None if audience.is_empty() => Err("the Hub's token names no Server"),
        None => Ok(true),
    }
}

fn rejected(rejected: Rejected) -> Failure {
    match rejected {
        Rejected::UnknownKey => "the Hub's token names a key it doesn't publish".into(),
        Rejected::Invalid(why) => format!("the Hub's token doesn't check out: {why}").into(),
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
