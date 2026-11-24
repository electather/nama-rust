//! The Relay (GLOSSARY.md) an Admin runs for clients that can't reach their Server directly, and
//! the Server's side of it (ADR 0057). The Server dials out to the Relay and shows a token the Hub
//! signed naming its own name (ADR 0011); the Relay keeps the connection waiting, and hands it the
//! next client whose TLS `ClientHello` names that name, or one under it, in its SNI. The Relay
//! reads only the `ClientHello` and copies bytes both ways, so it never decrypts the traffic: the
//! Server's own TLS (`tls.rs`) answers the client through it.
//!
//! The Relay runs as a process of its own (`nama-server relay`), with no Kernel and no Plugins, so
//! it fetches the Hub's keys itself; the Server asks for its token through the Plugin that links it
//! (ADR 0094).
//!
//! A Server's dial is the token's length as two big-endian bytes, then the token, then nothing
//! until the Relay starts sending it a client's bytes. Both share the Relay's one port, told apart
//! by their first byte.

use std::{
    collections::HashMap,
    io, iter,
    net::SocketAddr,
    pin::pin,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ED25519, UnparsedPublicKey};
use rustls::server::Acceptor;
use serde::{Deserialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::{Notify, mpsc},
    task::{JoinHandle, JoinSet},
    time::{Instant, timeout, timeout_at},
};

use crate::{db, diagnostics, link::Linking, tls};

/// The first byte of a TLS record carrying a handshake, which a client's connection starts with
/// (RFC 8446, section 5.1).
const HANDSHAKE: u8 = 0x16;

/// The longest token a dial may carry, short enough that a dial's first byte is never
/// [`HANDSHAKE`].
const MAX_TOKEN_LENGTH: usize = 4096;
const _: () = assert!(MAX_TOKEN_LENGTH >> 8 < HANDSHAKE as usize);

/// What a token for a Relay names as its audience, as the Hub signs it.
const AUDIENCE: &str = "relay";

/// How far past its `exp` a token still passes, for a Relay whose clock runs behind the Hub's.
const LEEWAY_SECS: i64 = 60;

/// How many connections a Server keeps waiting at its Relay, for clients that arrive together,
/// such as a browser opening several.
const WAITING_PER_SERVER: usize = 4;

/// How long a Server's connection waits at the Relay before the Server dials again with a fresh
/// token: under [`WAITS_AT_MOST`], so the Server always goes first.
// ponytail: no TCP keepalive, so a NAT that forgets an idle connection sooner leaves the Relay
// handing clients a dead one until this redial; send keepalives if Relays report such hangs.
const DIAL_AGAIN_AFTER: Duration = Duration::from_mins(4);

/// How long the Relay keeps a Server's connection waiting.
const WAITS_AT_MOST: Duration = Duration::from_mins(5);

/// How many connections the Relay keeps waiting for one Server's name; one more drops the oldest.
const MOST_WAITING: usize = 16;

/// How long a Server waits to dial again after a dial failed, or to ask again for a token.
const RETRY_AFTER: Duration = Duration::from_mins(1);

/// How long a Server dials with the same token, well within the 5 minutes the Hub signs it for.
const TOKEN_REUSED_FOR: Duration = Duration::from_mins(2);

/// How long a connection may take to connect, or to send its dial or its `ClientHello`, so one that
/// stalls doesn't hold a task for ever.
const OPENING_WITHIN: Duration = Duration::from_secs(30);

/// How long a client waits at the Relay for one of its Server's connections.
const CLIENT_WAITS_AT_MOST: Duration = Duration::from_secs(10);

/// The most bytes the Relay reads of a client before its `ClientHello` is whole.
const MAX_HELLO: usize = 64 * 1024;

/// How long the Relay checks tokens against the Hub's key set before fetching it again, and how
/// long it waits at least between fetches, so a token naming an unknown key can't flood the Hub.
const KEYS_KEPT_FOR: Duration = Duration::from_hours(1);
const KEYS_FETCHED_AT_MOST_EVERY: Duration = Duration::from_mins(1);

/// How long the Relay waits for the Hub's key set.
const HUB_ANSWERS_WITHIN: Duration = Duration::from_secs(10);

/// Runs a Relay on `listener`: it keeps each Server's connection waiting once its token checks out
/// against the key set of the Hub at `hub_url`, and passes each client's TLS to a connection of
/// the Server its SNI names. Runs until the task is dropped.
///
/// # Errors
///
/// When it can't make the client that fetches the Hub's key set.
#[expect(
    clippy::infinite_loop,
    reason = "a Relay serves until its task is dropped, as the Server's own accepting does"
)]
pub async fn serve(listener: TcpListener, hub_url: &str) -> io::Result<()> {
    let relay = Arc::new(Relay::new(hub_url)?);
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(Arc::clone(&relay).take(stream));
            }
            Err(err) => tls::pause_after(&err).await,
        }
    }
}

/// The Relay's state: the Hub's key set and the Servers' connections waiting for clients.
#[derive(Debug)]
struct Relay {
    keys: HubKeys,
    /// By the own name each Server's token names, newest last.
    waiting: Mutex<HashMap<String, Vec<Waiting>>>,
    /// Woken as a Server's connection starts waiting.
    arrived: Notify,
}

/// A Server's connection waiting for a client.
#[derive(Debug)]
struct Waiting {
    stream: TcpStream,
    until: Instant,
}

impl Waiting {
    /// Whether it may still carry a client: not kept too long, nor closed, nor sending, which a
    /// Server doesn't before a client's bytes reach it.
    fn is_open(&self) -> bool {
        Instant::now() < self.until
            && matches!(
                self.stream.try_read(&mut [0; 1]),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock
            )
    }
}

impl Relay {
    fn new(hub_url: &str) -> io::Result<Self> {
        Ok(Self {
            keys: HubKeys::new(hub_url)?,
            waiting: Mutex::default(),
            arrived: Notify::new(),
        })
    }

    /// Keeps `stream` waiting if it is a Server's dial, or carries it if it is a client's.
    async fn take(self: Arc<Self>, stream: TcpStream) {
        let mut first = [0; 1];
        if !matches!(
            timeout(OPENING_WITHIN, stream.peek(&mut first)).await,
            Ok(Ok(1))
        ) {
            return;
        }
        if first[0] == HANDSHAKE {
            self.carry(stream).await;
        } else {
            self.keep_waiting(stream).await;
        }
    }

    /// Keeps `server`'s connection waiting under the own name its token names, once the token
    /// checks out, or drops it.
    async fn keep_waiting(&self, mut server: TcpStream) {
        let Ok(Ok(token)) = timeout(OPENING_WITHIN, read_token(&mut server)).await else {
            return;
        };
        let Some(name) = self.keys.name_in(&token).await else {
            return;
        };
        {
            let mut waiting = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
            let queue = waiting.entry(name).or_default();
            queue.retain(Waiting::is_open);
            if queue.len() >= MOST_WAITING {
                queue.remove(0);
            }
            queue.push(Waiting {
                stream: server,
                until: Instant::now() + WAITS_AT_MOST,
            });
        }
        self.arrived.notify_waiters();
    }

    /// Passes `client`'s TLS, untouched both ways, to a connection of the Server its SNI names.
    async fn carry(&self, mut client: TcpStream) {
        let Ok(Ok((hello, server_name))) =
            timeout(OPENING_WITHIN, read_client_hello(&mut client)).await
        else {
            return;
        };
        // Each connection that fails at once is gone from the queue, so the next one is tried.
        while let Some(mut server) = self.server_for(&server_name).await {
            if server.write_all(&hello).await.is_ok() {
                // Either side closing ends it, and there is no one left to tell.
                let _: io::Result<_> =
                    tokio::io::copy_bidirectional(&mut client, &mut server).await;
                return;
            }
        }
    }

    /// A connection of the Server whose own name `server_name` is, or is under, once one waits,
    /// for up to [`CLIENT_WAITS_AT_MOST`].
    async fn server_for(&self, server_name: &str) -> Option<TcpStream> {
        let deadline = Instant::now() + CLIENT_WAITS_AT_MOST;
        loop {
            let mut arrived = pin!(self.arrived.notified());
            // Before looking, so a connection that starts waiting meanwhile still wakes it.
            arrived.as_mut().enable();
            if let Some(server) = self.take_waiting(server_name) {
                return Some(server);
            }
            timeout_at(deadline, arrived).await.ok()?;
        }
    }

    /// The newest open connection waiting under the own name `server_name` is, or is under.
    fn take_waiting(&self, server_name: &str) -> Option<TcpStream> {
        let mut waiting = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
        let own = names_under(server_name).find(|name| waiting.contains_key(*name))?;
        let queue = waiting.get_mut(own)?;
        iter::from_fn(|| queue.pop()).find_map(|server| server.is_open().then_some(server.stream))
    }
}

/// `server_name`, then each name it is under, such as `relay.abc.zone`, `abc.zone` and `zone`.
fn names_under(server_name: &str) -> impl Iterator<Item = &str> {
    iter::successors(Some(server_name), |name| {
        name.split_once('.').map(|(_, parent)| parent)
    })
}

/// The token a Server's dial on `server` carries.
async fn read_token(server: &mut TcpStream) -> io::Result<String> {
    let length = usize::from(server.read_u16().await?);
    if length > MAX_TOKEN_LENGTH {
        return Err(io::Error::other("no token is that long"));
    }
    let mut token = vec![0; length];
    server.read_exact(&mut token).await?;
    String::from_utf8(token).map_err(io::Error::other)
}

/// What `client` sent until its `ClientHello` was whole, and the server name it gives in its SNI,
/// in lowercase. Only the `ClientHello` is read, which TLS sends unencrypted.
async fn read_client_hello(client: &mut TcpStream) -> io::Result<(Vec<u8>, String)> {
    let mut acceptor = Acceptor::default();
    let mut hello = Vec::new();
    let mut buffer = [0; 4096];
    while hello.len() < MAX_HELLO {
        let read = client.read(&mut buffer).await?;
        let mut unread = buffer.get(..read).unwrap_or_default();
        if unread.is_empty() {
            break;
        }
        hello.extend_from_slice(unread);
        while !unread.is_empty() {
            if acceptor.read_tls(&mut unread)? == 0 {
                return Err(io::Error::other("the ClientHello doesn't fit"));
            }
            match acceptor.accept() {
                Ok(Some(accepted)) => {
                    let server_name = (accepted.client_hello().server_name())
                        .ok_or_else(|| io::Error::other("the client names no server"))?
                        .to_ascii_lowercase();
                    return Ok((hello, server_name));
                }
                Ok(None) => {}
                Err((err, _alert)) => return Err(io::Error::other(err)),
            }
        }
    }
    Err(io::ErrorKind::UnexpectedEof.into())
}

/// The Hub's key set, which the Relay checks Servers' tokens against (ADR 0098).
#[derive(Debug)]
struct HubKeys {
    url: String,
    client: reqwest::Client,
    /// The keys and when they were fetched, or last tried to be; none before the first fetch.
    fetched: tokio::sync::Mutex<Option<(Vec<Key>, Instant)>>,
}

#[derive(Deserialize)]
struct KeySet {
    keys: Vec<Key>,
}

#[derive(Debug, Clone, Deserialize)]
struct Key {
    kid: String,
    crv: String,
    x: String,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    aud: String,
    exp: i64,
}

/// Why a token didn't pass.
#[derive(Debug, PartialEq, Eq)]
enum Rejected {
    /// The key set has no key by the id the token names: fetch it again.
    UnknownKey,
    /// Malformed, badly signed, for another audience, or expired.
    Invalid,
}

impl HubKeys {
    fn new(hub_url: &str) -> io::Result<Self> {
        diagnostics::use_ring_for_tls();
        let client = (reqwest::Client::builder().timeout(HUB_ANSWERS_WITHIN))
            .build()
            .map_err(io::Error::other)?;
        Ok(Self {
            url: hub_url.trim_end_matches('/').to_owned(),
            client,
            fetched: tokio::sync::Mutex::default(),
        })
    }

    /// The own name a Server's `token` names, once one of the Hub's keys signed it for a Relay
    /// and it hasn't expired.
    async fn name_in(&self, token: &str) -> Option<String> {
        let mut fetched = self.fetched.lock().await;
        if fetched
            .as_ref()
            .is_none_or(|(_, at)| at.elapsed() > KEYS_KEPT_FOR)
        {
            self.fetch_into(&mut fetched).await;
        }
        let (keys, at) = fetched.as_ref()?;
        match verify(keys, token, db::now()) {
            Err(Rejected::UnknownKey) if at.elapsed() > KEYS_FETCHED_AT_MOST_EVERY => {
                self.fetch_into(&mut fetched).await;
                verify(&fetched.as_ref()?.0, token, db::now()).ok()
            }
            verified => verified.ok(),
        }
    }

    /// Fetches the Hub's keys into `fetched`, keeping those it held while the Hub doesn't answer.
    async fn fetch_into(&self, fetched: &mut Option<(Vec<Key>, Instant)>) {
        let keys = match self.fetch().await {
            Ok(key_set) => key_set.keys,
            Err(err) => {
                tracing::warn!("could not fetch the Hub's keys, so new Servers can't dial: {err}");
                fetched.take().map_or_else(Vec::new, |(keys, _)| keys)
            }
        };
        *fetched = Some((keys, Instant::now()));
    }

    async fn fetch(&self) -> io::Result<KeySet> {
        let url = format!("{}/.well-known/jwks.json", self.url);
        let fetched = async {
            let response = self.client.get(url).send().await?.error_for_status()?;
            response.bytes().await
        };
        let body = fetched.await.map_err(io::Error::other)?;
        serde_json::from_slice(&body).map_err(io::Error::other)
    }
}

/// The own name `token` names, if one of `keys` signed it for a Relay and it is unexpired at
/// `now`, in seconds since the Unix epoch.
// A copy of the Hub Plugin's check (`plugins/hub/src/token.rs`), which the Server can't depend on
// (ADR 0006): a fix to one belongs in both.
fn verify(keys: &[Key], token: &str, now: i64) -> Result<String, Rejected> {
    let (signed, signature) = token.rsplit_once('.').ok_or(Rejected::Invalid)?;
    let (header, claims) = signed.split_once('.').ok_or(Rejected::Invalid)?;
    let header: Header = decoded(header)?;
    // Only EdDSA: a token naming another algorithm, such as `none`, never passes.
    if header.alg != "EdDSA" {
        return Err(Rejected::Invalid);
    }
    let key = keys
        .iter()
        .find(|key| key.kid == header.kid && key.crv == "Ed25519")
        .ok_or(Rejected::UnknownKey)?;
    let public = URL_SAFE_NO_PAD
        .decode(&key.x)
        .map_err(|_not_base64| Rejected::Invalid)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_not_base64| Rejected::Invalid)?;
    UnparsedPublicKey::new(&ED25519, public)
        .verify(signed.as_bytes(), &signature)
        .map_err(|_unspecified| Rejected::Invalid)?;
    let claims: Claims = decoded(claims)?;
    let passes = claims.aud == AUDIENCE
        && !claims.sub.is_empty()
        && claims.exp.saturating_add(LEEWAY_SECS) >= now;
    passes.then_some(claims.sub).ok_or(Rejected::Invalid)
}

/// `part` of a token, base64url JSON, read as `T`.
fn decoded<T: DeserializeOwned>(part: &str) -> Result<T, Rejected> {
    let json = URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|_not_base64| Rejected::Invalid)?;
    serde_json::from_slice(&json).map_err(|_not_json| Rejected::Invalid)
}

/// Keeps [`WAITING_PER_SERVER`] connections waiting at `relay`, the Relay the Admin runs, each
/// dialed with a token the Plugin that links the Server gets through `linking`, and hands
/// `relayed` each that a client's bytes start arriving on, with the Relay's address as its peer.
/// A dial that fails is tried again after [`RETRY_AFTER`]. Runs until the task is aborted.
pub(crate) fn keep_dialing(
    relay: SocketAddr,
    linking: Linking,
    relayed: mpsc::Sender<(TcpStream, SocketAddr)>,
) -> JoinHandle<()> {
    let tokens = Arc::new(Tokens {
        linking,
        last: tokio::sync::Mutex::default(),
    });
    tokio::spawn(async move {
        // Dropping the set, as aborting this task does, aborts each.
        let mut dialing = JoinSet::new();
        for _ in 0..WAITING_PER_SERVER {
            dialing.spawn(keep_one_waiting(
                relay,
                Arc::clone(&tokens),
                relayed.clone(),
            ));
        }
        dialing.join_all().await;
    })
}

/// Keeps one connection waiting at `relay`, dialing again as each starts carrying a client, which
/// goes to `relayed`, or has waited [`DIAL_AGAIN_AFTER`], until serving stops.
async fn keep_one_waiting(
    relay: SocketAddr,
    tokens: Arc<Tokens>,
    relayed: mpsc::Sender<(TcpStream, SocketAddr)>,
) {
    loop {
        // Linking logs why there is none, if a Plugin links the Server.
        let Some(token) = tokens.get().await else {
            tokio::time::sleep(RETRY_AFTER).await;
            continue;
        };
        match wait_for_client(relay, &token).await {
            Ok(Some(client)) => {
                if relayed.send((client, relay)).await.is_err() {
                    return;
                }
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!("could not wait for clients at the Relay: {err}");
                tokio::time::sleep(RETRY_AFTER).await;
            }
        }
    }
}

/// A connection to `relay` dialed with `token`, once a client's bytes start arriving on it, or
/// `None` once it waited [`DIAL_AGAIN_AFTER`] for none.
async fn wait_for_client(relay: SocketAddr, token: &str) -> io::Result<Option<TcpStream>> {
    let dial = dial(token)?;
    let mut stream = timeout(OPENING_WITHIN, TcpStream::connect(relay))
        .await
        .map_err(|_elapsed| io::Error::from(io::ErrorKind::TimedOut))??;
    stream.write_all(&dial).await?;
    match timeout(DIAL_AGAIN_AFTER, stream.peek(&mut [0; 1])).await {
        Err(_waited) => Ok(None),
        Ok(Ok(0)) => Err(io::Error::other(
            "the Relay closed the connection, as it does with a token it doesn't take",
        )),
        Ok(Ok(_)) => Ok(Some(stream)),
        Ok(Err(err)) => Err(err),
    }
}

/// A Server's dial carrying `token`.
// ponytail: the token crosses in plain text and any Relay takes it, so whoever sees it can crowd
// out the Server's waiting connections for its 5 minutes, though never read its clients' TLS; bind
// it to one Relay and send it over TLS once the project runs Relays for everyone (ADR 0057).
fn dial(token: &str) -> io::Result<Vec<u8>> {
    let length = u16::try_from(token.len())
        .ok()
        .filter(|&length| usize::from(length) <= MAX_TOKEN_LENGTH)
        .ok_or_else(|| io::Error::other("the token is too long for a Relay"))?;
    Ok([&length.to_be_bytes()[..], token.as_bytes()].concat())
}

/// The Server's last token for its Relay, or the failure to get one, each kept a while so the
/// connections dialing share it.
#[derive(Debug)]
struct Tokens {
    linking: Linking,
    last: tokio::sync::Mutex<Option<(Option<String>, Instant)>>,
}

impl Tokens {
    /// A token for the Relay: the last one, for up to [`TOKEN_REUSED_FOR`], or else a new one,
    /// with `None` while the Plugin that links the Server gives none, asked again only after
    /// [`RETRY_AFTER`].
    async fn get(&self) -> Option<String> {
        let mut last = self.last.lock().await;
        if let Some((token, at)) = &*last {
            let kept_for = if token.is_some() {
                TOKEN_REUSED_FOR
            } else {
                RETRY_AFTER
            };
            if at.elapsed() < kept_for {
                return token.clone();
            }
        }
        let token = self.linking.relay_token().await.ok();
        *last = Some((token.clone(), Instant::now()));
        token
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
