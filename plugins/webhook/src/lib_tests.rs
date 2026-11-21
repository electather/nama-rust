use nama_pdk::extism_pdk::json::{self, Value};
use nama_pdk::{
    Notification, Notifier, Recipient, Response,
    nama_plugin_proto::nama::plugin::v1::{
        Event, Request, RequestState, RequestStateChanged, RequestedMovie, SettingKind,
    },
    services,
    testing::{Kernel, Sent, with},
};

use super::{exports::Webhook, manifest, signature};

/// Dune turning Available, with the link to its Request.
fn dune_available() -> Notification {
    Notification {
        event: Event {
            event: Some(RequestStateChanged::default().into()),
            ..Event::default()
        }
        .into(),
        request: Request {
            id: 7,
            movie_or_season: RequestedMovie {
                title: "Dune".to_owned(),
                ..RequestedMovie::default()
            }
            .into(),
            state: RequestState::REQUEST_STATE_AVAILABLE.into(),
            ..Request::default()
        }
        .into(),
        request_url: Some("https://nama.example.com/requests/7".to_owned()),
        ..Notification::default()
    }
}

/// The Webhook's URL setting at `http://hooks.test/nama`, answering every post with `status`.
fn webhook(status: u16) -> Kernel {
    Kernel::new()
        .setting("url", "http://hooks.test/nama")
        .http(move |_request, _body| Ok(Response::new(status, "")))
}

/// Delivers Dune turning Available to the channel through `kernel`, then the one post it sent.
fn deliver_one(kernel: &mut Kernel) -> Result<Sent, nama_pdk::extism_pdk::Error> {
    with(kernel, || {
        Webhook::deliver(dune_available(), Recipient::Channel(Box::default()))
    })?;
    let [sent] = kernel.sent() else {
        panic!(
            "the webhook was sent {} posts, not one",
            kernel.sent().len()
        );
    };
    Ok(sent.clone())
}

#[test]
fn a_notification_posts_one_json_body_with_its_event_request_link_and_text_to_the_url() {
    let mut kernel = webhook(204);

    let sent = deliver_one(&mut kernel).unwrap();

    assert_eq!(sent.request.url, "http://hooks.test/nama");
    assert_eq!(sent.request.method.as_deref(), Some("POST"));
    assert_eq!(
        sent.request.headers.get("Content-Type").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(sent.request.headers.get("X-Nama-Signature"), None);
    let posted: Value = json::from_slice(&sent.body).unwrap();
    assert!(posted["event"].is_object(), "{posted}");
    assert_eq!(posted["request"]["state"], "REQUEST_STATE_AVAILABLE");
    assert_eq!(posted["request_url"], "https://nama.example.com/requests/7");
    assert_eq!(posted["text"], "Dune is now Available.");
}

#[test]
fn with_a_key_set_the_body_carries_its_hmac_sha256_signature() {
    let mut kernel = webhook(204).setting("secret", "Jefe");

    let sent = deliver_one(&mut kernel).unwrap();

    assert_eq!(
        sent.request.headers.get("X-Nama-Signature"),
        Some(&signature("Jefe", &sent.body).unwrap())
    );
}

#[test]
fn a_refusal_fails_naming_its_status() {
    let mut kernel = webhook(500);

    let error = deliver_one(&mut kernel).unwrap_err();

    assert_eq!(error.to_string(), "the webhook failed with HTTP status 500");
}

#[test]
fn the_signature_is_the_hex_hmac_sha256_of_the_body() {
    // RFC 4231, test case 2.
    assert_eq!(
        signature("Jefe", b"what do ya want for nothing?").unwrap(),
        "sha256=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn manifest_declares_a_url_and_an_optional_secret_and_no_personal_settings_or_connector() {
    let manifest = manifest();
    let settings: Vec<_> = manifest
        .settings
        .iter()
        .map(|field| (field.key.as_str(), field.kind.as_known(), field.required))
        .collect();

    assert_eq!(
        settings,
        [
            ("url", Some(SettingKind::SETTING_KIND_URL), true),
            ("secret", Some(SettingKind::SETTING_KIND_SECRET), false),
        ]
    );
    assert_eq!(manifest.personal_settings, []);
    assert_eq!(manifest.points, [nama_pdk::point(services::NOTIFIER)]);
}
