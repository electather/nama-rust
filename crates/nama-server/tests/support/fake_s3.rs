//! A local stand-in for an S3 bucket, holding its objects in memory, so the S3 Plugin's tests
//! through the API need no network and no credentials. It serves one path-style bucket,
//! [`BUCKET`]: PUT, GET, DELETE and HEAD of an object, and `ListObjectsV2`. It checks each
//! request's AWS Signature V4, presigned in its query as the Plugin signs, with its own made-up
//! key, and refuses any other request as S3 does.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    net::SocketAddr,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use connectrpc::client::HttpClient;
use hmac::{Hmac, KeyInit as _, Mac as _};
use nama_proto::nama::v1::{
    EnablePluginRequest, PluginServiceClient, SettingValue, UpdatePluginSettingsRequest,
    setting_value,
};
use sha2::{Digest as _, Sha256};
use tokio::{net::TcpListener, sync::Semaphore, task::JoinHandle};

use crate::server::TestServer;

/// The S3 Plugin's id.
pub(crate) const S3: &str = "s3";

/// The one bucket the fake serves.
const BUCKET: &str = "media";

/// The region the fake signs for.
const REGION: &str = "eu-west-1";

/// The access key ID the fake knows: made up, as is [`SECRET`].
const ACCESS_KEY_ID: &str = "AKIAFAKEFAKEFAKE";

/// The secret access key the fake knows.
const SECRET: &str = "fake-s3-secret-for-tests-only";

/// What the Plugin is set to put before every key, so the fake also sees that it does.
const PREFIX: &str = "nama/";

/// Where the Server's write, read and delete check of a new store writes. A hold lets it through,
/// so a switch still starts.
const PROBE_DIR: &str = "store-probe/";

/// S3's XML namespace, which a listing's root element is in.
const S3_XML_NS: &str = "http://s3.amazonaws.com/doc/2006-03-01/";

/// A fake S3 serving on a local port until dropped.
pub(crate) struct FakeS3 {
    addr: SocketAddr,
    bucket: Arc<Bucket>,
    server: JoinHandle<()>,
}

/// The bucket's objects, and how the fake answers writes.
struct Bucket {
    /// By their whole key, prefix included.
    objects: Mutex<BTreeMap<String, Vec<u8>>>,
    /// Answers every PUT with 403 while set.
    refusing_writes: AtomicBool,
    /// Keeps each PUT of an image waiting for [`FakeS3::release_one`] while set.
    holding: AtomicBool,
    /// The PUTs that arrived while [`Self::holding`].
    held: AtomicUsize,
    /// A permit lets one held PUT through.
    released: Semaphore,
}

impl FakeS3 {
    /// Starts serving an empty bucket on a free local port.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn start() -> Self {
        let bucket = Arc::new(Bucket {
            objects: Mutex::default(),
            refusing_writes: AtomicBool::default(),
            holding: AtomicBool::default(),
            held: AtomicUsize::default(),
            released: Semaphore::new(0),
        });
        let app = Router::new()
            .fallback(answer)
            .with_state(Arc::clone(&bucket));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _: Result<(), _> = axum::serve(listener, app).await;
        });
        Self {
            addr,
            bucket,
            server,
        }
    }

    /// The S3 Plugin's settings for this fake, path-style, signing with [`SECRET`].
    fn settings(&self) -> Vec<(String, SettingValue)> {
        let text = |text: &str| setting_value::Value::TextValue(text.to_owned());
        [
            ("endpoint", text(&format!("http://{}", self.addr))),
            ("region", text(REGION)),
            ("bucket", text(BUCKET)),
            ("prefix", text(PREFIX)),
            ("access_key_id", text(ACCESS_KEY_ID)),
            ("secret_access_key", text(SECRET)),
            ("path_style", setting_value::Value::BoolValue(true)),
        ]
        .into_iter()
        .map(|(key, value)| {
            let value = SettingValue {
                value: Some(value),
                ..Default::default()
            };
            (key.to_owned(), value)
        })
        .collect()
    }

    /// Has `server`'s Admin set the S3 Plugin up against this fake and enable it.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) async fn enable_on(&self, server: &TestServer) {
        let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
        plugins
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: S3.to_owned(),
                settings: self.settings().into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        plugins
            .enable_plugin(EnablePluginRequest {
                id: S3.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The objects the Plugin stored, by the Server's key: the bucket's key less the prefix.
    pub(crate) fn objects(&self) -> BTreeMap<String, Vec<u8>> {
        lock(&self.bucket.objects)
            .iter()
            .filter_map(|(key, bytes)| Some((key.strip_prefix(PREFIX)?.to_owned(), bytes.clone())))
            .collect()
    }

    /// Has every PUT answer 403 from now on, as a bucket the key may only read does.
    pub(crate) fn refuse_writes(&self) {
        self.bucket.refusing_writes.store(true, Ordering::SeqCst);
    }

    /// Keeps each PUT of an image waiting from now on, until [`Self::release_one`] lets it
    /// through. The Server's check of a new store isn't kept waiting.
    pub(crate) fn hold_writes(&self) {
        self.bucket.holding.store(true, Ordering::SeqCst);
    }

    /// How many PUTs have arrived since [`Self::hold_writes`], those let through included.
    pub(crate) fn held(&self) -> usize {
        self.bucket.held.load(Ordering::SeqCst)
    }

    /// Lets one held PUT through, or the next to arrive.
    pub(crate) fn release_one(&self) {
        self.bucket.released.add_permits(1);
    }
}

impl Drop for FakeS3 {
    /// Stops serving: dropping a task's handle alone would leave it running.
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn answer(
    State(bucket): State<Arc<Bucket>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let query: Vec<(String, String)> =
        Query::try_from_uri(&uri).map_or_default(|Query(query)| query);
    let host = headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or_default();
    if let Err(code) = verify(&method, uri.path(), &query, host, SECRET) {
        return error(StatusCode::FORBIDDEN, code);
    }
    let Some(path) = uri.path().strip_prefix(&format!("/{BUCKET}")) else {
        return error(StatusCode::NOT_FOUND, "NoSuchBucket");
    };
    let key = percent_decoded(path.strip_prefix('/').unwrap_or(path));
    if key.is_empty() {
        return if method == Method::GET {
            list(&bucket, &query)
        } else {
            error(StatusCode::METHOD_NOT_ALLOWED, "MethodNotAllowed")
        };
    }
    match method {
        Method::PUT => put(&bucket, key, body.to_vec()).await,
        Method::GET => match lock(&bucket.objects).get(&key) {
            Some(bytes) => bytes.clone().into_response(),
            None => error(StatusCode::NOT_FOUND, "NoSuchKey"),
        },
        // An answer to a HEAD has no body, a refusal's included.
        Method::HEAD if lock(&bucket.objects).contains_key(&key) => StatusCode::OK.into_response(),
        Method::HEAD => StatusCode::NOT_FOUND.into_response(),
        Method::DELETE => {
            lock(&bucket.objects).remove(&key);
            StatusCode::NO_CONTENT.into_response()
        }
        _ => error(StatusCode::METHOD_NOT_ALLOWED, "MethodNotAllowed"),
    }
}

async fn put(bucket: &Bucket, key: String, bytes: Vec<u8>) -> Response {
    if bucket.refusing_writes.load(Ordering::SeqCst) {
        return error(StatusCode::FORBIDDEN, "AccessDenied");
    }
    let probe = key
        .strip_prefix(PREFIX)
        .is_some_and(|key| key.starts_with(PROBE_DIR));
    if bucket.holding.load(Ordering::SeqCst) && !probe {
        bucket.held.fetch_add(1, Ordering::SeqCst);
        match bucket.released.acquire().await {
            Ok(permit) => permit.forget(),
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "ServiceUnavailable"),
        }
    }
    lock(&bucket.objects).insert(key, bytes);
    StatusCode::OK.into_response()
}

/// Checks a request's presigned signature, as AWS documents it for query authentication, or says
/// the error code S3 refuses it with. `path` is the request's path as sent, which is what is
/// signed.
fn verify(
    method: &Method,
    path: &str,
    query: &[(String, String)],
    host: &str,
    secret: &str,
) -> Result<(), &'static str> {
    let param = |name: &str| {
        query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let (Some(signature), Some("AWS4-HMAC-SHA256"), Some(credential), Some(date)) = (
        param("X-Amz-Signature"),
        param("X-Amz-Algorithm"),
        param("X-Amz-Credential"),
        param("X-Amz-Date"),
    ) else {
        return Err("AccessDenied");
    };
    let [key_id, day, region, "s3", "aws4_request"] = credential.split('/').collect::<Vec<_>>()[..]
    else {
        return Err("AuthorizationQueryParametersError");
    };
    if key_id != ACCESS_KEY_ID {
        return Err("InvalidAccessKeyId");
    }
    if region != REGION || param("X-Amz-SignedHeaders") != Some("host") {
        return Err("AuthorizationQueryParametersError");
    }
    let mut signed: Vec<_> = query
        .iter()
        .filter(|(name, _)| name != "X-Amz-Signature")
        .map(|(name, value)| (uri_encoded(name), uri_encoded(value)))
        .collect();
    signed.sort();
    let signed: Vec<_> = signed
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    let canonical = format!(
        "{method}\n{path}\n{}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD",
        signed.join("&"),
    );
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{date}\n{day}/{region}/s3/aws4_request\n{}",
        hex(&Sha256::digest(canonical.as_bytes()))
    );
    let signing_key = [day, region, "s3", "aws4_request"]
        .iter()
        .fold(format!("AWS4{secret}").into_bytes(), |key, part| {
            hmac(&key, part.as_bytes())
        });
    if hex(&hmac(&signing_key, to_sign.as_bytes())) == signature {
        Ok(())
    } else {
        Err("SignatureDoesNotMatch")
    }
}

/// A `ListObjectsV2` page, its continuation token the first key of the next page.
fn list(bucket: &Bucket, query: &[(String, String)]) -> Response {
    let param = |name: &str| {
        query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    if param("list-type") != Some("2") {
        return error(StatusCode::BAD_REQUEST, "InvalidArgument");
    }
    let prefix = param("prefix").unwrap_or_default();
    let max_keys = param("max-keys")
        .and_then(|max| max.parse().ok())
        .unwrap_or(1000_usize)
        .min(1000);
    let from = param("continuation-token").unwrap_or_default();
    let url_encoded = param("encoding-type") == Some("url");
    let encode = |text: &str| {
        if url_encoded {
            uri_encoded(text)
        } else {
            text.to_owned()
        }
    };
    let objects = lock(&bucket.objects);
    let listed: Vec<_> = objects
        .iter()
        .filter(|(key, _)| key.starts_with(prefix) && key.as_str() >= from)
        .collect();
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><ListBucketResult xmlns="{S3_XML_NS}"><Name>{BUCKET}</Name><Prefix>{}</Prefix><MaxKeys>{max_keys}</MaxKeys>"#,
        encode(prefix)
    );
    for (key, bytes) in listed.iter().take(max_keys) {
        let _ = write!(
            xml,
            "<Contents><Key>{}</Key><Size>{}</Size></Contents>",
            encode(key),
            bytes.len()
        );
    }
    if let Some((next, _)) = listed.get(max_keys) {
        let _ = write!(
            xml,
            "<IsTruncated>true</IsTruncated><NextContinuationToken>{}</NextContinuationToken>",
            encode(next)
        );
    } else {
        xml += "<IsTruncated>false</IsTruncated>";
    }
    if url_encoded {
        xml += "<EncodingType>url</EncodingType>";
    }
    xml += "</ListBucketResult>";
    ([(header::CONTENT_TYPE, "application/xml")], xml).into_response()
}

/// S3's refusal with `code`, in the XML body it gives one.
fn error(status: StatusCode, code: &str) -> Response {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Error><Code>{code}</Code><Message>The fake S3 refused the request.</Message><RequestId>FAKE</RequestId></Error>"#
    );
    (status, [(header::CONTENT_TYPE, "application/xml")], xml).into_response()
}

#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes a key of any length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

/// `text` URI-encoded as `SigV4`'s canonical query string encodes it: everything but the
/// unreserved characters.
fn uri_encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            byte => format!("%{byte:02X}"),
        })
        .collect()
}

/// `text` with its `%XX` escapes decoded, as S3 reads a key from a path.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let escaped = bytes
            .get(at + 1..at + 3)
            .filter(|_| byte == b'%')
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        if let Some(escaped) = escaped {
            decoded.push(escaped);
            at += 3;
        } else {
            decoded.push(byte);
            at += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Locks `mutex`, even one a panicking test poisoned.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
