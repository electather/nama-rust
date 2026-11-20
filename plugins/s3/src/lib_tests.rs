use std::{cell::RefCell, collections::HashMap, rc::Rc};

use nama_pdk::{
    BLOB_MAX_BYTES, BlobStore, Response,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    services,
    testing::{Kernel, with},
};
use url::Url;

use super::{
    S3, check,
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

/// A Kernel holding `settings`, answering no request.
fn configured(settings: &HashMap<&'static str, String>) -> Kernel {
    (settings.iter()).fold(Kernel::new(), |kernel, (key, value)| {
        kernel.setting(key, value)
    })
}

/// An empty bucket accepting [`SECRET`], shared with the Kernel that sends it requests.
fn bucket() -> Rc<RefCell<FakeS3>> {
    Rc::new(RefCell::new(FakeS3::new(SECRET)))
}

/// A Kernel holding `settings`, whose requests `fake` answers.
fn kernel(settings: &HashMap<&'static str, String>, fake: &Rc<RefCell<FakeS3>>) -> Kernel {
    let fake = Rc::clone(fake);
    configured(settings).http(move |request, body| Ok(fake.borrow_mut().answer(request, body)))
}

/// Calls the Plugin with the settings for [`SECRET`], against `fake`.
fn s3<R>(fake: &Rc<RefCell<FakeS3>>, call: impl FnOnce() -> R) -> R {
    with(&mut kernel(&settings(SECRET), fake), call)
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
    let fake = bucket();

    let (got, exists) = s3(&fake, || {
        S3::put("posters/1.jpg".to_owned(), b"poster".to_vec())?;
        Ok::<_, nama_pdk::extism_pdk::Error>((
            S3::get("posters/1.jpg".to_owned())?,
            S3::exists("posters/1.jpg".to_owned())?,
        ))
    })
    .unwrap();

    assert_eq!(
        fake.borrow()
            .objects
            .get("nama/posters/1.jpg")
            .map(Vec::as_slice),
        Some(&b"poster"[..])
    );
    assert_eq!(got.as_deref(), Some(&b"poster"[..]));
    assert!(exists);
}

#[test]
fn a_put_replaces_the_object_at_its_key() {
    let fake = bucket();

    let got = s3(&fake, || {
        S3::put("a".to_owned(), b"old".to_vec())?;
        S3::put("a".to_owned(), b"new".to_vec())?;
        S3::get("a".to_owned())
    })
    .unwrap();

    assert_eq!(got.as_deref(), Some(&b"new"[..]));
}

#[test]
fn a_missing_key_is_not_found_and_deleting_it_succeeds() {
    let fake = bucket();

    let (got, exists, deleted) = s3(&fake, || {
        (
            S3::get("missing.jpg".to_owned()).unwrap(),
            S3::exists("missing.jpg".to_owned()).unwrap(),
            S3::delete("missing.jpg".to_owned()),
        )
    });

    assert_eq!(got, None);
    assert!(!exists);
    deleted.unwrap();
}

#[test]
fn a_deleted_object_is_gone() {
    let fake = bucket();

    let exists = s3(&fake, || {
        S3::put("logo.svg".to_owned(), b"<svg/>".to_vec())?;
        S3::delete("logo.svg".to_owned())?;
        S3::exists("logo.svg".to_owned())
    })
    .unwrap();

    assert!(fake.borrow().objects.is_empty());
    assert!(!exists);
}

#[test]
fn a_listing_goes_over_pages_of_1000_keys_without_the_prefix() {
    let fake = bucket();
    {
        let objects = &mut fake.borrow_mut().objects;
        for n in 0..1001 {
            objects.insert(format!("nama/posters/{n:04}.jpg"), Vec::new());
        }
        objects.insert("nama/backdrops/1.jpg".to_owned(), Vec::new());
        objects.insert("other/posters/1.jpg".to_owned(), Vec::new());
    }

    let (first, second) = s3(&fake, || {
        let first = S3::list("posters/".to_owned(), String::new()).unwrap();
        let second = S3::list("posters/".to_owned(), first.next_page_token.clone()).unwrap();
        (first, second)
    });

    assert_eq!(first.keys.len(), 1000);
    assert_eq!(first.keys[0], "posters/0000.jpg");
    assert_ne!(first.next_page_token, "");
    assert_eq!(second.keys, ["posters/1000.jpg"]);
    assert_eq!(second.next_page_token, "");
}

#[test]
fn a_wrong_secret_is_refused_with_s3s_error_code() {
    let fake = bucket();
    let mut kernel = kernel(&settings("not the secret"), &fake);

    let refused = with(&mut kernel, || {
        [
            S3::put("a.jpg".to_owned(), b"a".to_vec()).unwrap_err(),
            S3::get("a.jpg".to_owned()).unwrap_err(),
            check().unwrap_err(),
        ]
    });

    assert!(fake.borrow().objects.is_empty());
    for err in refused {
        assert!(err.to_string().contains("SignatureDoesNotMatch"), "{err}");
    }
}

#[test]
fn checking_the_settings_lists_one_key() {
    let mut kernel = kernel(&settings(SECRET), &bucket());

    with(&mut kernel, check).unwrap();

    let [sent] = kernel.sent() else {
        panic!("the check sent {} requests, not one", kernel.sent().len());
    };
    let asked = Url::parse(&sent.request.url).unwrap();
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
    let mut kernel = configured(&settings).http(|_request, _body| Ok(Response::new(404, "")));

    with(&mut kernel, || S3::exists("a.jpg".to_owned())).unwrap();

    let asked = &kernel.sent()[0].request;
    let url = Url::parse(&asked.url).unwrap();
    assert_eq!(asked.method.as_deref(), Some("HEAD"));
    assert_eq!(url.host_str(), Some("media.s3.eu-west-1.amazonaws.com"));
    assert_eq!(url.path(), "/nama/a.jpg");
}

#[test]
fn a_missing_bucket_is_an_error_not_a_missing_key() {
    let mut settings = settings(SECRET);
    settings.insert("bucket", "elsewhere".to_owned());

    let err = with(&mut kernel(&settings, &bucket()), || {
        S3::get("a.jpg".to_owned())
    })
    .unwrap_err();

    assert!(err.to_string().contains("NoSuchBucket"), "{err}");
}

#[test]
fn an_object_over_the_cap_is_refused_without_a_request() {
    let mut kernel = configured(&settings(SECRET));

    let err = with(&mut kernel, || {
        S3::put("huge".to_owned(), vec![0; BLOB_MAX_BYTES + 1])
    })
    .unwrap_err();

    assert!(err.to_string().contains("20 MiB"), "{err}");
    assert_eq!(kernel.sent(), []);
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

        let checked = with(&mut configured(&settings), check);

        assert!(checked.is_err(), "{key} {value:?}");
    }
}

#[test]
fn without_a_key_requests_go_unsigned() {
    let mut unsigned = settings(SECRET);
    unsigned.remove("access_key_id");
    unsigned.remove("secret_access_key");
    let mut kernel = configured(&unsigned).http(|_request, _body| Ok(Response::new(404, "")));

    with(&mut kernel, || S3::exists("a.jpg".to_owned())).unwrap();

    let url = Url::parse(&kernel.sent()[0].request.url).unwrap();
    assert!(
        url.query_pairs().all(|(name, _)| name != "X-Amz-Signature"),
        "{url}"
    );
}
