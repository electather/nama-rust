use std::{
    collections::HashMap,
    time::{Duration, UNIX_EPOCH},
};

use nama_pdk::{
    BLOB_MAX_BYTES,
    extism_pdk::Error,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    services,
};
use url::Url;

use super::{
    Answer, Call, Store, check,
    fake_s3::{ACCESS_KEY_ID, BUCKET, FakeS3, REGION},
    manifest,
};

const SECRET: &str = "fake/secret+key";

/// The settings reaching [`FakeS3`]'s bucket with `secret`, keeping the files under `nama/`.
fn settings(secret: &str) -> HashMap<&'static str, String> {
    HashMap::from([
        ("endpoint", "http://fake-s3.test:9000".to_owned()),
        ("region", REGION.to_owned()),
        ("bucket", BUCKET.to_owned()),
        ("prefix", "nama/".to_owned()),
        ("access_key_id", ACCESS_KEY_ID.to_owned()),
        ("secret_access_key", secret.to_owned()),
        ("path_style", "true".to_owned()),
    ])
}

fn store_with(settings: &HashMap<&'static str, String>) -> Result<Store, Error> {
    Store::new(
        |key| Ok(settings.get(key).cloned()),
        UNIX_EPOCH + Duration::from_hours(500_000),
    )
}

fn store() -> Store {
    store_with(&settings(SECRET)).unwrap()
}

/// Sends a call to `fake`.
fn to(fake: &mut FakeS3) -> impl FnOnce(Call) -> Result<Answer, Error> + '_ {
    |call| Ok(fake.answer(call))
}

#[test]
fn manifest_serves_the_blob_store_with_its_settings() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::BLOB_STORE]);
    assert_eq!(manifest.allowed_hosts, ["*.amazonaws.com"]);
    let settings: Vec<_> = manifest
        .settings
        .iter()
        .map(|field| {
            let default = field.default_value.as_option();
            let default = default.and_then(|default| default.value.clone());
            (
                field.key.as_str(),
                field.kind.as_known(),
                field.required,
                default,
            )
        })
        .collect();
    let text = Some(SettingKind::SETTING_KIND_TEXT);
    assert_eq!(
        settings,
        [
            ("endpoint", Some(SettingKind::SETTING_KIND_URL), true, None),
            (
                "region",
                text,
                false,
                Some(Value::TextValue("us-east-1".to_owned()))
            ),
            ("bucket", text, true, None),
            ("prefix", text, false, None),
            ("access_key_id", text, false, None),
            (
                "secret_access_key",
                Some(SettingKind::SETTING_KIND_SECRET),
                false,
                None
            ),
            (
                "path_style",
                Some(SettingKind::SETTING_KIND_BOOL),
                false,
                Some(Value::BoolValue(false))
            ),
        ]
    );
}

#[test]
fn an_object_put_is_got_back_under_the_prefix() {
    let mut fake = FakeS3::new(SECRET);
    let store = store();

    store
        .put("posters/1.jpg", b"poster".to_vec(), to(&mut fake))
        .unwrap();

    assert_eq!(
        fake.objects.get("nama/posters/1.jpg").map(Vec::as_slice),
        Some(&b"poster"[..])
    );
    let got = store.get("posters/1.jpg", to(&mut fake)).unwrap();
    assert_eq!(got.as_deref(), Some(&b"poster"[..]));
    assert!(store.exists("posters/1.jpg", to(&mut fake)).unwrap());
}

#[test]
fn a_put_replaces_the_object_at_its_key() {
    let mut fake = FakeS3::new(SECRET);
    let store = store();

    store.put("a", b"old".to_vec(), to(&mut fake)).unwrap();
    store.put("a", b"new".to_vec(), to(&mut fake)).unwrap();

    let got = store.get("a", to(&mut fake)).unwrap();
    assert_eq!(got.as_deref(), Some(&b"new"[..]));
}

#[test]
fn a_missing_key_is_not_found_and_deleting_it_succeeds() {
    let mut fake = FakeS3::new(SECRET);
    let store = store();

    assert_eq!(store.get("missing.jpg", to(&mut fake)).unwrap(), None);
    assert!(!store.exists("missing.jpg", to(&mut fake)).unwrap());
    store.delete("missing.jpg", to(&mut fake)).unwrap();
}

#[test]
fn a_deleted_object_is_gone() {
    let mut fake = FakeS3::new(SECRET);
    let store = store();
    store
        .put("logo.svg", b"<svg/>".to_vec(), to(&mut fake))
        .unwrap();

    store.delete("logo.svg", to(&mut fake)).unwrap();

    assert!(fake.objects.is_empty());
    assert!(!store.exists("logo.svg", to(&mut fake)).unwrap());
}

#[test]
fn a_listing_goes_over_pages_of_1000_keys_without_the_prefix() {
    let mut fake = FakeS3::new(SECRET);
    for n in 0..1001 {
        fake.objects
            .insert(format!("nama/posters/{n:04}.jpg"), Vec::new());
    }
    fake.objects
        .insert("nama/backdrops/1.jpg".to_owned(), Vec::new());
    fake.objects
        .insert("other/posters/1.jpg".to_owned(), Vec::new());
    let store = store();

    let first = store.list("posters/", "", 1000, to(&mut fake)).unwrap();
    let second = store
        .list("posters/", &first.next_page_token, 1000, to(&mut fake))
        .unwrap();

    assert_eq!(first.keys.len(), 1000);
    assert_eq!(first.keys[0], "posters/0000.jpg");
    assert_ne!(first.next_page_token, "");
    assert_eq!(second.keys, ["posters/1000.jpg"]);
    assert_eq!(second.next_page_token, "");
}

#[test]
fn a_wrong_secret_is_refused_with_s3s_error_code() {
    let mut fake = FakeS3::new(SECRET);
    let store = store_with(&settings("not the secret")).unwrap();

    let put = store
        .put("a.jpg", b"a".to_vec(), to(&mut fake))
        .unwrap_err();
    let got = store.get("a.jpg", to(&mut fake)).unwrap_err();
    let checked = check(&store, to(&mut fake)).unwrap_err();

    assert!(fake.objects.is_empty());
    for err in [put, got, checked] {
        assert!(err.to_string().contains("SignatureDoesNotMatch"), "{err}");
    }
}

#[test]
fn checking_the_settings_lists_one_key() {
    let mut fake = FakeS3::new(SECRET);
    let mut asked = None;

    check(&store(), |call| {
        asked = Some(call.url.clone());
        Ok(fake.answer(call))
    })
    .unwrap();

    let asked = asked.unwrap();
    let param = |name| asked.query_pairs().find(|(key, _)| key == name);
    assert_eq!(param("max-keys").unwrap().1, "1");
    assert_eq!(param("prefix").unwrap().1, "nama/");
}

#[test]
fn the_fake_checks_signatures_as_aws_publishes_them() {
    // AWS's example of a presigned GET, from "Authenticating Requests: Using Query Parameters
    // (AWS Signature Version 4)" in the Amazon S3 API Reference.
    let mut fake = FakeS3::new("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    fake.access_key_id = "AKIAIOSFODNN7EXAMPLE".to_owned();
    fake.region = "us-east-1".to_owned();
    let signed = |signature: &str| {
        Url::parse(&format!(
            "https://examplebucket.s3.amazonaws.com/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256\
             &X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request\
             &X-Amz-Date=20130524T000000Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host\
             &X-Amz-Signature={signature}"
        ))
        .unwrap()
    };
    let published = "aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404";

    assert_eq!(fake.verify("GET", &signed(published)), Ok(()));
    let tampered = published.replacen('a', "b", 1);
    assert_eq!(
        fake.verify("GET", &signed(&tampered)),
        Err("SignatureDoesNotMatch")
    );
    assert_eq!(
        fake.verify("PUT", &signed(published)),
        Err("SignatureDoesNotMatch")
    );
}

#[test]
fn virtual_hosted_addressing_puts_the_bucket_in_the_host() {
    let mut settings = settings(SECRET);
    settings.insert("endpoint", "https://s3.eu-west-1.amazonaws.com".to_owned());
    settings.remove("path_style");
    let mut asked = None;

    store_with(&settings)
        .unwrap()
        .exists("a.jpg", |call| {
            asked = Some((call.method, call.url));
            Ok(Answer {
                status: 404,
                body: Vec::new(),
            })
        })
        .unwrap();

    let (method, url) = asked.unwrap();
    assert_eq!(method, "HEAD");
    assert_eq!(url.host_str(), Some("media.s3.eu-west-1.amazonaws.com"));
    assert_eq!(url.path(), "/nama/a.jpg");
}

#[test]
fn a_missing_bucket_is_an_error_not_a_missing_key() {
    let mut fake = FakeS3::new(SECRET);
    let mut settings = settings(SECRET);
    settings.insert("bucket", "elsewhere".to_owned());

    let err = store_with(&settings)
        .unwrap()
        .get("a.jpg", to(&mut fake))
        .unwrap_err();

    assert!(err.to_string().contains("NoSuchBucket"), "{err}");
}

#[test]
fn an_object_over_the_cap_is_refused_without_a_request() {
    let err = store()
        .put("huge", vec![0; BLOB_MAX_BYTES + 1], |_| {
            panic!("an object over the cap was sent")
        })
        .unwrap_err();

    assert!(err.to_string().contains("20 MiB"), "{err}");
}

#[test]
fn settings_need_an_endpoint_a_bucket_and_both_halves_of_a_key_or_neither() {
    for (key, value) in [
        ("endpoint", None),
        ("endpoint", Some("not a url")),
        ("endpoint", Some("ftp://fake-s3.test")),
        ("bucket", Some(" ")),
        ("secret_access_key", None),
    ] {
        let mut settings = settings(SECRET);
        match value {
            Some(value) => settings.insert(key, value.to_owned()),
            None => settings.remove(key),
        };

        assert!(store_with(&settings).is_err(), "{key} {value:?}");
    }

    let mut unsigned = settings(SECRET);
    unsigned.remove("access_key_id");
    unsigned.remove("secret_access_key");
    assert!(store_with(&unsigned).unwrap().credentials.is_none());
}
