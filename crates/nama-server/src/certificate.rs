//! The certificate for the Server's names on the Hub (ADR 0057): one for its own name and the
//! wildcard under it, which its LAN names fall under, from an ACME CA, Let's Encrypt unless
//! `Config::acme_directory` names another. The CA checks a DNS-01 challenge the Plugin that links
//! the Server has its service publish, but the Server makes the certificate's key itself, so the
//! service never holds it. The certificate is kept in the data directory and served from startup,
//! whether or not the service answers, and renewed a month before it expires. While the Server has
//! none it serves plain HTTP alone (`tls.rs`).

use std::{
    fmt::Display,
    future::Future,
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::{Arc, PoisonError, RwLock},
    time::Duration,
};

use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, NewAccount,
    NewOrder, Order, OrderStatus, RetryPolicy,
};
use nama_plugin_api::server_link::Reachability;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject as _},
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};
use serde::{Deserialize, Serialize};
use tokio::{task::JoinHandle, time::MissedTickBehavior};
use url::Url;
use x509_parser::{certificate::X509Certificate, extensions::GeneralName, prelude::FromDer as _};

use crate::{
    db,
    link::{self, Link, Linking},
    server_settings::ServerUrl,
};

/// The file in the data directory holding the certificate's key, then its chain, leaf first.
const CERTIFICATE_FILE: &str = "certificate.pem";

/// The file in the data directory holding the Server's account at the ACME CA.
const ACCOUNT_FILE: &str = "acme-account.json";

/// How often the Server looks at its names, so it gets a certificate within this long of being
/// named, and its Server URL follows its Link.
const EVERY: Duration = Duration::from_mins(1);

/// How long before it expires a certificate is renewed: a third of the 90 days Let's Encrypt's
/// last, as it advises.
const RENEW_BEFORE_SECS: i64 = 30 * 24 * 60 * 60;

/// How long the Server waits after failing to get a certificate before it tries again, well within
/// Let's Encrypt's limit of 5 failed validations per name an hour.
const RETRY_AFTER_SECS: i64 = 60 * 60;

/// How long the published challenge gets to reach the zone's nameservers before the CA checks it.
const PUBLISHED_WITHIN: Duration = Duration::from_secs(10);

/// How long the Server waits on a step at the ACME CA. instant-acme bounds no request, and one
/// that hung would stop renewing, and the default Server URL with it, for good. Above the 30
/// seconds its polling takes at most.
const CA_WITHIN: Duration = Duration::from_mins(2);

/// The certificate the Server serves over TLS, once it has one. Clones share it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Certificate(Arc<RwLock<Option<Held>>>);

#[derive(Debug)]
struct Held {
    /// The names it is for, such as `<id>.namadirect.net` and `*.<id>.namadirect.net`.
    names: Vec<String>,
    /// In seconds since the Unix epoch.
    expires_at: i64,
    key: Arc<CertifiedKey>,
}

impl ResolvesServerCert for Certificate {
    fn resolve(&self, _client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let held = self.0.read().unwrap_or_else(PoisonError::into_inner);
        held.as_ref().map(|held| Arc::clone(&held.key))
    }
}

impl Certificate {
    /// Whether it is for `own` and lasts more than `margin` seconds after `now`.
    fn lasts(&self, own: &str, now: i64, margin: i64) -> bool {
        let held = self.0.read().unwrap_or_else(PoisonError::into_inner);
        held.as_ref().is_some_and(|held| {
            held.names.iter().any(|name| name == own)
                && held.expires_at > now.saturating_add(margin)
        })
    }

    fn hold(&self, held: Held) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = Some(held);
    }

    /// Serves the certificate kept in `data_dir`, if there is one. One that can't be read is
    /// logged and left for the next renewal to replace.
    pub(crate) async fn load(&self, data_dir: &Path) {
        let pem = match tokio::fs::read(data_dir.join(CERTIFICATE_FILE)).await {
            Ok(pem) => pem,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return,
            Err(err) => {
                tracing::warn!("could not read the certificate for the Server's names: {err}");
                return;
            }
        };
        match parse(&pem) {
            Ok(held) => self.hold(held),
            Err(err) => tracing::warn!("the certificate kept for the Server's names {err}"),
        }
    }
}

/// Why the Server got no certificate.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("failed at the ACME CA: {0}")]
    Acme(#[from] instant_acme::Error),
    #[error("couldn't have the challenge published: {0}")]
    Challenge(#[from] link::Error),
    #[error("the ACME CA offered no DNS-01 challenge")]
    NoDnsChallenge,
    #[error("the ACME CA left the order {0:?}")]
    NotReady(OrderStatus),
    #[error("couldn't keep it: {0}")]
    Io(#[from] io::Error),
    #[error("the ACME CA didn't answer within {CA_WITHIN:?}")]
    CaSilent(#[from] tokio::time::error::Elapsed),
    #[error("couldn't keep the ACME account: {0}")]
    Account(#[from] serde_json::Error),
    #[error("isn't usable: {0}")]
    Unusable(String),
}

fn unusable(err: impl Display) -> Error {
    Error::Unusable(err.to_string())
}

/// Every [`EVERY`], looks at the Server's names through `linking`. While the service has the
/// Server Linked and named and `certificate` won't last another [`RENEW_BEFORE_SECS`] for its own
/// name, it gets one from the ACME CA at `acme_directory`, waiting [`RETRY_AFTER_SECS`] after a
/// failure. Meanwhile it keeps `server_url`'s default at the Server's public name. A certificate it
/// gets is served with `certificate` and kept in `data_dir`. Runs until the task is aborted.
pub(crate) fn keep_renewed(
    certificate: Certificate,
    linking: Linking,
    server_url: ServerUrl,
    data_dir: PathBuf,
    acme_directory: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EVERY);
        // A renewal can outlast a tick, and the ticks it missed needn't follow at once.
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut next_try = i64::MIN;
        loop {
            ticks.tick().await;
            let now = db::now();
            let named = named(&linking).await;
            let public = (named.as_ref()).and_then(|found| public_url(found, &certificate, now));
            server_url.set_default(public);
            let Some(own) = named.as_ref().and_then(|found| found.names.first()) else {
                continue;
            };
            if now < next_try || certificate.lasts(own, now, RENEW_BEFORE_SECS) {
                continue;
            }
            if let Err(err) = renew(&certificate, &linking, &data_dir, &acme_directory, own).await {
                tracing::warn!("could not get a certificate for the Server's names: {err}");
                next_try = now.saturating_add(RETRY_AFTER_SECS);
            }
        }
    })
}

/// What the service found of how clients reach the Server, while it has the Server Linked and
/// named.
async fn named(linking: &Linking) -> Option<Reachability> {
    match linking.server_link().await {
        Ok(Link::Linked {
            reachability: Some(found),
            ..
        }) if !found.names.is_empty() => Some(found),
        _ => None,
    }
}

/// The Server URL while the Admin sets none: `https://<own name>:<public port>`, while
/// `certificate` is for its own name and the service reached its public port, which the name
/// points at only then (ADR 0104).
fn public_url(found: &Reachability, certificate: &Certificate, now: i64) -> Option<String> {
    let own = found.names.first()?;
    let port = Url::parse(&found.address).ok()?.port_or_known_default()?;
    (found.reachable && certificate.lasts(own, now, 0)).then(|| format!("https://{own}:{port}"))
}

/// Gets a certificate for `own` and the wildcard under it from the ACME CA at `acme_directory`,
/// serves it with `certificate` and keeps it in `data_dir`.
async fn renew(
    certificate: &Certificate,
    linking: &Linking,
    data_dir: &Path,
    acme_directory: &str,
    own: &str,
) -> Result<(), Error> {
    let identifiers = [
        Identifier::Dns(own.to_owned()),
        Identifier::Dns(format!("*.{own}")),
    ];
    let mut order = at_ca(async {
        let account = account(data_dir, acme_directory).await?;
        Ok(account.new_order(&NewOrder::new(&identifiers)).await?)
    })
    .await?;
    let validated = validate(&mut order, linking).await;
    // Whether or not it worked, so the zone keeps no stale challenge. `ask` logs a failure, and the
    // next order replaces what this leaves.
    let _: Result<(), _> = linking.set_certificate_challenge(Vec::new()).await;
    validated?;
    let (key, chain) = at_ca(async {
        let key = order.finalize().await?;
        Ok((key, order.poll_certificate(&RetryPolicy::default()).await?))
    })
    .await?;
    let pem = format!("{key}{chain}");
    let held = parse(pem.as_bytes())?;
    keep_secret(data_dir.join(CERTIFICATE_FILE), pem.into_bytes()).await?;
    certificate.hold(held);
    Ok(())
}

/// Has the service publish `order`'s DNS-01 challenges, and waits for the CA to check them.
async fn validate(order: &mut Order, linking: &Linking) -> Result<(), Error> {
    let values = at_ca(challenges(order)).await?;
    // Not bounded like the CA's steps: a change at the service isn't cut (`link.rs`).
    linking.set_certificate_challenge(values).await?;
    tokio::time::sleep(PUBLISHED_WITHIN).await;
    at_ca(checked(order)).await
}

/// Tells the CA `order`'s challenges are published, and waits for it to check them.
async fn checked(order: &mut Order) -> Result<(), Error> {
    let mut authorizations = order.authorizations();
    while let Some(authorization) = authorizations.next().await {
        let mut authorization = authorization?;
        if authorization.status == AuthorizationStatus::Pending {
            let challenge = authorization.challenge(ChallengeType::Dns01);
            challenge.ok_or(Error::NoDnsChallenge)?.set_ready().await?;
        }
    }
    let status = order.poll_ready(&RetryPolicy::default()).await?;
    if status == OrderStatus::Ready {
        Ok(())
    } else {
        Err(Error::NotReady(status))
    }
}

/// The values of the DNS-01 challenges `order` waits on.
async fn challenges(order: &mut Order) -> Result<Vec<String>, Error> {
    let mut values = Vec::new();
    let mut authorizations = order.authorizations();
    while let Some(authorization) = authorizations.next().await {
        let mut authorization = authorization?;
        // One the account proved within the last month is valid already.
        if authorization.status != AuthorizationStatus::Pending {
            continue;
        }
        let challenge = authorization.challenge(ChallengeType::Dns01);
        values.push(
            challenge
                .ok_or(Error::NoDnsChallenge)?
                .key_authorization()
                .dns_value(),
        );
    }
    Ok(values)
}

/// `step` at the ACME CA, failed after [`CA_WITHIN`].
async fn at_ca<T>(step: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
    tokio::time::timeout(CA_WITHIN, step).await?
}

/// The Server's account at an ACME CA, as kept in [`ACCOUNT_FILE`].
#[derive(Serialize, Deserialize)]
struct KeptAccount {
    /// The CA's directory, so a change of CA makes a new account.
    directory: String,
    credentials: AccountCredentials,
}

/// The Server's account at the ACME CA at `directory`, made and kept in `data_dir` the first time.
/// Making it agrees to the CA's terms, such as Let's Encrypt's Subscriber Agreement. A kept one
/// that can't be read is made again, as it never could be.
async fn account(data_dir: &Path, directory: &str) -> Result<Account, Error> {
    let path = data_dir.join(ACCOUNT_FILE);
    match tokio::fs::read(&path).await {
        Ok(kept) => match serde_json::from_slice::<KeptAccount>(&kept) {
            Ok(kept) if kept.directory == directory => {
                let builder = Account::builder()?;
                return Ok(builder.from_credentials(kept.credentials).await?);
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!("the ACME account kept can't be read, so one is made: {err}");
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    let new = NewAccount {
        contact: &[],
        terms_of_service_agreed: true,
        only_return_existing: false,
    };
    let (account, credentials) = (Account::builder()?)
        .create(&new, directory.to_owned(), None)
        .await?;
    let kept = KeptAccount {
        directory: directory.to_owned(),
        credentials,
    };
    keep_secret(path, serde_json::to_vec(&kept)?).await?;
    Ok(account)
}

/// The certificate in `pem`: its key, then its chain, leaf first.
fn parse(pem: &[u8]) -> Result<Held, Error> {
    let key = PrivateKeyDer::from_pem_slice(pem).map_err(unusable)?;
    let chain = (CertificateDer::pem_slice_iter(pem))
        .collect::<Result<Vec<_>, _>>()
        .map_err(unusable)?;
    let (names, expires_at) = {
        let leaf = chain
            .first()
            .ok_or_else(|| unusable("it holds no certificate"))?;
        let (_, leaf) = X509Certificate::from_der(leaf).map_err(unusable)?;
        let alternative = leaf.subject_alternative_name().map_err(unusable)?;
        let names = (alternative.iter())
            .flat_map(|alternative| &alternative.value.general_names)
            .filter_map(|name| {
                if let GeneralName::DNSName(name) = name {
                    Some((*name).to_owned())
                } else {
                    None
                }
            })
            .collect();
        (names, leaf.validity().not_after.timestamp())
    };
    let signing = rustls::crypto::ring::sign::any_supported_type(&key).map_err(unusable)?;
    Ok(Held {
        names,
        expires_at,
        key: Arc::new(CertifiedKey::new(chain, signing)),
    })
}

/// Replaces `path` with `bytes` whole, readable by the Server's user alone, as `tempfile` makes
/// its files: they hold a private key.
async fn keep_secret(path: PathBuf, bytes: Vec<u8>) -> io::Result<()> {
    let writing = tokio::task::spawn_blocking(move || {
        let folder = path.parent().unwrap_or_else(|| Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(folder)?;
        file.write_all(&bytes)?;
        file.persist(&path).map_err(|failed| failed.error)?;
        Ok(())
    });
    writing.await.map_err(io::Error::other)?
}

#[cfg(test)]
#[path = "certificate_tests.rs"]
mod tests;
