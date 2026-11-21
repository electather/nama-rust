//! The S3 Installed Plugin: a blob store keeping the Server's stored files in an S3 or
//! S3-compatible bucket. The Server builds it for wasm32 and bundles it in its binary.

use std::time::{Duration, SystemTime};

use instant_xml::FromXml;
use nama_pdk::{
    BLOB_MAX_BYTES, BlobPage, BlobStore, Manifest, OutsideService, Response, SettingField,
    SettingKind,
    extism_pdk::{Error, HttpRequest},
    services, setting_default, setting_field, setting_value,
};
use rusty_s3::{Bucket, BucketError, Credentials, S3Action, UrlStyle, actions::ListObjectsV2};
use url::Url;

const ENDPOINT: &str = "endpoint";
const REGION: &str = "region";
const BUCKET: &str = "bucket";
const PREFIX: &str = "prefix";
const ACCESS_KEY_ID: &str = "access_key_id";
const SECRET_ACCESS_KEY: &str = "secret_access_key";
const PATH_STYLE: &str = "path_style";

const DEFAULT_REGION: &str = "us-east-1";

/// How long a signed URL stays valid: past the 60 seconds a blob store call may run, with room
/// for a Server clock a little off S3's.
const SIGNED_FOR: Duration = Duration::from_mins(15);

/// The most keys a page of `list` holds: the contract's limit, and S3's own.
const PAGE_KEYS: usize = 1000;

/// S3 words its refusals in XML, which [`refused`] reads, so only its name is needed here.
const S3_SERVICE: OutsideService = OutsideService::new("S3");

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, CheckedSettings, FnResult, Manifest, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        super::check()?;
        Ok(Buffa(CheckedSettings::default()))
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        name: "S3".to_owned(),
        description: "Keeps the Server's stored files, such as its images, in an S3 or \
                      S3-compatible bucket."
            .to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::BLOB_STORE)],
        // A virtual-hosted bucket is reached at `<bucket>.<endpoint's host>`, which isn't the
        // endpoint's own host; an S3-compatible store needs path-style addressing for that reason.
        allowed_hosts: vec!["*.amazonaws.com".to_owned()],
        settings: vec![
            SettingField {
                required: true,
                ..setting_field(
                    ENDPOINT,
                    "Endpoint",
                    "Where the store's S3 API is, such as https://s3.eu-west-1.amazonaws.com or \
                     your MinIO server's address.",
                    SettingKind::SETTING_KIND_URL,
                )
            },
            SettingField {
                default_value: setting_default(setting_value::Value::TextValue(
                    DEFAULT_REGION.to_owned(),
                )),
                ..setting_field(
                    REGION,
                    "Region",
                    "The bucket's region, such as eu-west-1.",
                    SettingKind::SETTING_KIND_TEXT,
                )
            },
            SettingField {
                required: true,
                ..setting_field(
                    BUCKET,
                    "Bucket",
                    "The bucket the files go in.",
                    SettingKind::SETTING_KIND_TEXT,
                )
            },
            setting_field(
                PREFIX,
                "Prefix",
                "Put before every key, such as nama/ to keep the files in a folder of the bucket.",
                SettingKind::SETTING_KIND_TEXT,
            ),
            setting_field(
                ACCESS_KEY_ID,
                "Access key ID",
                "The ID of an access key that may read, write, delete and list the bucket.",
                SettingKind::SETTING_KIND_TEXT,
            ),
            setting_field(
                SECRET_ACCESS_KEY,
                "Secret access key",
                "The secret of that access key.",
                SettingKind::SETTING_KIND_SECRET,
            ),
            SettingField {
                default_value: setting_default(setting_value::Value::BoolValue(false)),
                ..setting_field(
                    PATH_STYLE,
                    "Path-style addressing",
                    "Reach the bucket at the endpoint's own host, as most S3-compatible stores, \
                     such as MinIO, need.",
                    SettingKind::SETTING_KIND_BOOL,
                )
            },
        ],
        ..nama_pdk::manifest("s3", env!("CARGO_PKG_VERSION"))
    }
}

/// Serves the blob store from the bucket the settings name.
struct S3;

impl BlobStore for S3 {
    fn put(key: String, bytes: Vec<u8>) -> Result<(), Error> {
        store()?.put(&key, bytes)
    }

    fn get(key: String) -> Result<Option<Vec<u8>>, Error> {
        store()?.get(&key)
    }

    fn delete(key: String) -> Result<(), Error> {
        store()?.delete(&key)
    }

    fn list(prefix: String, page_token: String) -> Result<BlobPage, Error> {
        store()?.list(&prefix, &page_token, PAGE_KEYS)
    }

    fn exists(key: String) -> Result<bool, Error> {
        store()?.exists(&key)
    }
}

nama_pdk::blob_store!(S3);

/// The bucket the Server's settings for this call name, signing at the Server's time.
fn store() -> Result<Store, Error> {
    Store::new(nama_pdk::now())
}

/// Proves the settings by listing one key, as `check_settings()` does.
fn check() -> Result<(), Error> {
    store()?.list("", "", 1).map(drop)
}

/// The bucket the settings name, and how to sign a request to it.
struct Store {
    bucket: Bucket,
    credentials: Option<Credentials>,
    prefix: String,
    now: jiff::Timestamp,
}

impl Store {
    /// The bucket the settings name, signing at `now`.
    ///
    /// # Errors
    /// If a required setting is missing, the endpoint isn't a URL a bucket can be reached at, or
    /// only one of the access key's ID and secret is set.
    fn new(now: SystemTime) -> Result<Self, Error> {
        let text = |key| -> Result<Option<String>, Error> {
            Ok(nama_pdk::setting(key)?.map(|value| value.trim().to_owned()))
        };
        let required = |key| -> Result<String, Error> {
            Ok(nama_pdk::required_setting(key)?.trim().to_owned())
        };
        let endpoint = required(ENDPOINT)?;
        let endpoint = Url::parse(&endpoint)
            .map_err(|err| Error::msg(format!("the endpoint {endpoint} isn't a URL: {err}")))?;
        let name = required(BUCKET)?;
        let region = text(REGION)?.unwrap_or_else(|| DEFAULT_REGION.to_owned());
        let style = if text(PATH_STYLE)?.as_deref() == Some("true") {
            UrlStyle::Path
        } else {
            UrlStyle::VirtualHost
        };
        let bucket = Bucket::new(endpoint, style, name.clone(), region).map_err(|err| {
            Error::msg(match err {
                BucketError::UnsupportedScheme => "the endpoint isn't an http or https URL".into(),
                BucketError::MissingHost => "the endpoint has no host".into(),
                BucketError::ParseError(err) => format!(
                    "the bucket {name} can't be part of a host name ({err}); turn on \
                     path-style addressing"
                ),
            })
        })?;
        // Without a key the requests go unsigned, as a public bucket takes them.
        let credentials = match (text(ACCESS_KEY_ID)?, text(SECRET_ACCESS_KEY)?) {
            (Some(key), Some(secret)) => Some(Credentials::new(key, secret)),
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                return Err(Error::msg(
                    "set both the access key ID and the secret access key, or neither",
                ));
            }
        };
        let now = jiff::Timestamp::try_from(now)
            .map_err(|err| Error::msg(format!("the Server's clock is out of range: {err}")))?;
        Ok(Self {
            bucket,
            credentials,
            prefix: text(PREFIX)?.unwrap_or_default(),
            now,
        })
    }

    /// Stores `bytes` at `key`, replacing any object there.
    fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), Error> {
        if bytes.len() > BLOB_MAX_BYTES {
            return Err(Error::msg(format!(
                "{key} is over the {} MiB an object may be, so it wasn't stored",
                BLOB_MAX_BYTES >> 20
            )));
        }
        let object = self.object(key);
        let action = self.bucket.put_object(self.credentials.as_ref(), &object);
        let answer = self.send(&action, Some(bytes))?;
        match answer.status_code() {
            200..300 => Ok(()),
            _ => Err(refused(&format!("store {key}"), &answer)),
        }
    }

    /// The object at `key`, `None` if there is none.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let object = self.object(key);
        let action = self.bucket.get_object(self.credentials.as_ref(), &object);
        let answer = self.send(&action, None)?;
        match answer.status_code() {
            200..300 => Ok(Some(answer.into_body())),
            // A missing bucket is the settings' fault, not a missing object.
            404 if s3_error(answer.body()).is_none_or(|err| err.code != "NoSuchBucket") => Ok(None),
            _ => Err(refused(&format!("read {key}"), &answer)),
        }
    }

    /// Deletes the object at `key`; a missing one is already deleted.
    fn delete(&self, key: &str) -> Result<(), Error> {
        let object = self.object(key);
        let action = self
            .bucket
            .delete_object(self.credentials.as_ref(), &object);
        let answer = self.send(&action, None)?;
        match answer.status_code() {
            // S3 answers 204 for a missing object too, but some S3-compatible stores answer 404.
            200..300 => Ok(()),
            404 if s3_error(answer.body()).is_none_or(|err| err.code != "NoSuchBucket") => Ok(()),
            _ => Err(refused(&format!("delete {key}"), &answer)),
        }
    }

    /// Whether an object is at `key`.
    fn exists(&self, key: &str) -> Result<bool, Error> {
        let object = self.object(key);
        let action = self.bucket.head_object(self.credentials.as_ref(), &object);
        // A HEAD answer has no body, so a refusal says its status only.
        let answer = self.send(&action, None)?;
        match answer.status_code() {
            200..300 => Ok(true),
            404 => Ok(false),
            _ => Err(refused(&format!("say whether {key} exists"), &answer)),
        }
    }

    /// A page of at most `max_keys` keys starting with `prefix`, going on from `page_token`.
    fn list(&self, prefix: &str, page_token: &str, max_keys: usize) -> Result<BlobPage, Error> {
        let mut action = self.bucket.list_objects_v2(self.credentials.as_ref());
        action.with_prefix(self.object(prefix));
        if !page_token.is_empty() {
            action.with_continuation_token(page_token);
        }
        action.with_max_keys(max_keys);
        let answer = self.send(&action, None)?;
        if !(200..300).contains(&answer.status_code()) {
            return Err(refused(
                &format!("list the keys starting with {prefix:?}"),
                &answer,
            ));
        }
        let listed = std::str::from_utf8(answer.body())
            .ok()
            .and_then(|body| ListObjectsV2::parse_response(body).ok())
            .ok_or_else(|| Error::msg("S3 answered a listing with something unexpected"))?;
        Ok(BlobPage {
            keys: listed
                .contents
                .into_iter()
                .filter_map(|object| Some(object.key.strip_prefix(&self.prefix)?.to_owned()))
                .collect(),
            next_page_token: listed.next_continuation_token.unwrap_or_default(),
            ..BlobPage::default()
        })
    }

    /// The bucket's key for the Server's `key`.
    fn object(&self, key: &str) -> String {
        format!("{}{key}", self.prefix)
    }

    /// Sends `action`, signed in its URL at [`Self::now`], so it needs no headers.
    fn send<'a, A: S3Action<'a>>(
        &self,
        action: &A,
        body: Option<Vec<u8>>,
    ) -> Result<Response, Error> {
        let url = action.sign_with_time(SIGNED_FOR, &self.now);
        let request = HttpRequest::new(url.as_str()).with_method(A::METHOD.to_str());
        S3_SERVICE.send(&request, body)
    }
}

/// The error S3 answered with, in the XML body it gives every refusal but a HEAD's.
#[derive(Debug, FromXml)]
#[xml(rename = "Error")]
struct S3Error {
    #[xml(rename = "Code")]
    code: String,
    #[xml(rename = "Message")]
    message: Option<String>,
}

fn s3_error(body: &[u8]) -> Option<S3Error> {
    instant_xml::from_str(std::str::from_utf8(body).ok()?).ok()
}

/// The error for S3 refusing to `action`, with S3's error code and message if it gave them.
fn refused(action: &str, answer: &Response) -> Error {
    match s3_error(answer.body()) {
        Some(S3Error {
            code,
            message: Some(message),
        }) => Error::msg(format!("S3 refused to {action}: {code}: {message}")),
        Some(S3Error {
            code,
            message: None,
        }) => Error::msg(format!("S3 refused to {action}: {code}")),
        None => Error::msg(format!(
            "S3 refused to {action} with HTTP status {}",
            answer.status_code()
        )),
    }
}

#[cfg(test)]
mod fake_s3;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
