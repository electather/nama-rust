//! An in-process stand-in for S3, so the tests need no network and no credentials. It serves one
//! path-style bucket, [`BUCKET`], and checks each request's AWS Signature V4, presigned in its
//! query, with the secret it was given, as S3 does.

use std::{collections::BTreeMap, fmt::Write as _};

use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::{Digest as _, Sha256};
use url::Url;

use super::{Answer, Call};

/// The access key ID the fake knows.
pub(super) const ACCESS_KEY_ID: &str = "AKIAFAKEFAKEFAKE";

/// The one bucket the fake serves.
pub(super) const BUCKET: &str = "media";

/// What [`FakeS3::new`] serves under, which it signs for.
pub(super) const REGION: &str = "eu-west-1";

/// S3's XML namespace, which a listing's root element is in.
const S3_XML_NS: &str = "http://s3.amazonaws.com/doc/2006-03-01/";

/// An S3 bucket in memory.
pub(super) struct FakeS3 {
    /// The access key ID it knows, [`ACCESS_KEY_ID`] unless a test changes it.
    pub(super) access_key_id: String,
    secret: String,
    /// The region it signs for, [`REGION`] unless a test changes it.
    pub(super) region: String,
    /// The bucket's objects, by their whole key.
    pub(super) objects: BTreeMap<String, Vec<u8>>,
}

impl FakeS3 {
    /// An empty bucket in [`REGION`] accepting requests signed with `secret`.
    pub(super) fn new(secret: &str) -> Self {
        Self {
            access_key_id: ACCESS_KEY_ID.to_owned(),
            secret: secret.to_owned(),
            region: REGION.to_owned(),
            objects: BTreeMap::new(),
        }
    }

    /// S3's answer to `call`.
    pub(super) fn answer(&mut self, call: Call) -> Answer {
        let mut answer = self.answer_with_body(call.method, &call.url, call.body);
        // S3 answers a HEAD with no body, refusals included.
        if call.method == "HEAD" {
            answer.body.clear();
        }
        answer
    }

    fn answer_with_body(&mut self, method: &str, url: &Url, body: Option<Vec<u8>>) -> Answer {
        if let Err(code) = self.verify(method, url) {
            return error(403, code);
        }
        let Some(key) = url.path().strip_prefix(&format!("/{BUCKET}/")) else {
            return error(404, "NoSuchBucket");
        };
        let key = percent_decoded(key);
        match (method, key.is_empty()) {
            ("GET", true) => self.list(url),
            ("PUT", false) => {
                self.objects.insert(key, body.unwrap_or_default());
                answer(200, Vec::new())
            }
            ("GET", false) => match self.objects.get(&key) {
                Some(bytes) => answer(200, bytes.clone()),
                None => error(404, "NoSuchKey"),
            },
            ("HEAD", false) if self.objects.contains_key(&key) => answer(200, Vec::new()),
            ("HEAD", false) => error(404, "NoSuchKey"),
            ("DELETE", false) => {
                self.objects.remove(&key);
                answer(204, Vec::new())
            }
            _ => error(405, "MethodNotAllowed"),
        }
    }

    /// Checks `url`'s presigned signature for `method`, as AWS documents it for query
    /// authentication, or says the error code S3 refuses it with.
    pub(super) fn verify(&self, method: &str, url: &Url) -> Result<(), &'static str> {
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let param = |name: &str| {
            pairs
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
        let [key_id, day, region, "s3", "aws4_request"] =
            credential.split('/').collect::<Vec<_>>()[..]
        else {
            return Err("AuthorizationQueryParametersError");
        };
        if key_id != self.access_key_id {
            return Err("InvalidAccessKeyId");
        }
        if region != self.region || param("X-Amz-SignedHeaders") != Some("host") {
            return Err("AuthorizationQueryParametersError");
        }

        let mut query: Vec<_> = pairs
            .iter()
            .filter(|(name, _)| name != "X-Amz-Signature")
            .map(|(name, value)| (uri_encoded(name), uri_encoded(value)))
            .collect();
        query.sort();
        let query: Vec<_> = query
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        let host = match (url.host_str(), url.port()) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            (host, None) => host.unwrap_or_default().to_owned(),
            (None, Some(_)) => return Err("AccessDenied"),
        };
        let canonical = format!(
            "{method}\n{}\n{}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD",
            url.path(),
            query.join("&"),
        );
        let to_sign = format!(
            "AWS4-HMAC-SHA256\n{date}\n{day}/{region}/s3/aws4_request\n{}",
            hex(&Sha256::digest(canonical.as_bytes()))
        );
        let signing_key = [day, region, "s3", "aws4_request"].iter().fold(
            format!("AWS4{}", self.secret).into_bytes(),
            |signing, part| hmac(&signing, part.as_bytes()),
        );
        if hex(&hmac(&signing_key, to_sign.as_bytes())) == signature {
            Ok(())
        } else {
            Err("SignatureDoesNotMatch")
        }
    }

    /// A `ListObjectsV2` page, its continuation token the first key of the next page.
    fn list(&self, url: &Url) -> Answer {
        let param = |name: &str| {
            url.query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
        };
        if param("list-type").as_deref() != Some("2") {
            return error(400, "InvalidArgument");
        }
        let prefix = param("prefix").unwrap_or_default();
        let max_keys = param("max-keys")
            .and_then(|max| max.parse().ok())
            .unwrap_or(1000_usize)
            .min(1000);
        let from = param("continuation-token").unwrap_or_default();
        let keys: Vec<_> = self
            .objects
            .keys()
            .filter(|key| key.starts_with(&prefix) && **key >= from)
            .collect();
        let encode = |text: &str| {
            if param("encoding-type").as_deref() == Some("url") {
                uri_encoded(text)
            } else {
                text.to_owned()
            }
        };
        let mut xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><ListBucketResult xmlns="{S3_XML_NS}"><Name>{BUCKET}</Name><Prefix>{}</Prefix><MaxKeys>{max_keys}</MaxKeys>"#,
            encode(&prefix)
        );
        for key in keys.iter().take(max_keys) {
            let _ = write!(
                xml,
                "<Contents><Key>{}</Key><Size>1</Size></Contents>",
                encode(key)
            );
        }
        if let Some(next) = keys.get(max_keys) {
            let _ = write!(
                xml,
                "<IsTruncated>true</IsTruncated><NextContinuationToken>{next}</NextContinuationToken>"
            );
        } else {
            xml += "<IsTruncated>false</IsTruncated>";
        }
        if param("encoding-type").as_deref() == Some("url") {
            xml += "<EncodingType>url</EncodingType>";
        }
        xml += "</ListBucketResult>";
        answer(200, xml.into_bytes())
    }
}

fn answer(status: u16, body: Vec<u8>) -> Answer {
    Answer { status, body }
}

/// S3's refusal with `code`, in the XML body it gives one.
fn error(status: u16, code: &str) -> Answer {
    answer(
        status,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Error><Code>{code}</Code><Message>The fake S3 refused the request.</Message><RequestId>FAKE</RequestId></Error>"#
        )
        .into_bytes(),
    )
}

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
    while at < bytes.len() {
        let escaped = bytes
            .get(at + 1..at + 3)
            .filter(|_| bytes[at] == b'%')
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        if let Some(byte) = escaped {
            decoded.push(byte);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
