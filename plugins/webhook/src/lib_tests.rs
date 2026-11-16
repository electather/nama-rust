use nama_pdk::extism_pdk::json::{self, Value};
use nama_pdk::{
    Notification,
    nama_plugin_proto::nama::plugin::v1::{
        Event, Request, RequestState, RequestStateChanged, RequestedMovie, SettingKind,
    },
    services,
};

use super::{body, manifest, signature};

#[test]
fn the_body_mirrors_the_notification_with_a_line_of_text() {
    let notification = Notification {
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
    };

    let posted: Value =
        json::from_slice(&body(&notification, "Dune is now Available.").unwrap()).unwrap();

    assert_eq!(posted["request"]["state"], "REQUEST_STATE_AVAILABLE");
    assert!(posted["event"].is_object(), "{posted}");
    assert_eq!(posted["request_url"], "https://nama.example.com/requests/7");
    assert_eq!(posted["text"], "Dune is now Available.");
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
