use nama_plugin_api::contract::DeviceKind;

use super::{from_user_agent, kind_from_user_agent};

#[test]
fn names_common_browsers_and_systems() {
    let cases = [
        (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            "Chrome on macOS",
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
            "Edge on Windows",
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
            "Safari on iPhone",
        ),
        (
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
            "Chrome on Android",
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0",
            "Firefox on Linux",
        ),
        ("curl/8.7.1", "Unknown device"),
    ];
    for (user_agent, name) in cases {
        assert_eq!(from_user_agent(Some(user_agent)), name, "{user_agent}");
    }
    assert_eq!(from_user_agent(None), "Unknown device");
}

#[test]
fn tells_tvs_tablets_phones_and_computers_apart() {
    let cases = [
        (
            "Mozilla/5.0 (SMART-TV; LINUX; Tizen 6.5) AppleWebKit/537.36 (KHTML, like Gecko) 85.0.4183.93/6.5 TV Safari/537.36",
            DeviceKind::DEVICE_KIND_TV,
        ),
        (
            "Mozilla/5.0 (Linux; Android 9; AFTMM Build/PS7633) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            DeviceKind::DEVICE_KIND_TV,
        ),
        (
            "Mozilla/5.0 (iPad; CPU OS 17_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Mobile/15E148 Safari/604.1",
            DeviceKind::DEVICE_KIND_TABLET,
        ),
        (
            "Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            DeviceKind::DEVICE_KIND_TABLET,
        ),
        (
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
            DeviceKind::DEVICE_KIND_PHONE,
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
            DeviceKind::DEVICE_KIND_PHONE,
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0",
            DeviceKind::DEVICE_KIND_COMPUTER,
        ),
        ("curl/8.7.1", DeviceKind::DEVICE_KIND_UNSPECIFIED),
    ];
    for (user_agent, kind) in cases {
        assert_eq!(kind_from_user_agent(Some(user_agent)), kind, "{user_agent}");
    }
}

#[tokio::test]
async fn device_kinds_answers_each_device_the_server_still_has() {
    use std::sync::Arc;

    use super::super::{NewDevice, Permissions, Users, hash_token, insert_device, insert_user};
    use crate::{db::Database, plugins::ExtensionPoints};

    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let user = db
        .write(|tx| Box::pin(async move { insert_user(tx, "Root", Permissions::ADMIN).await }))
        .await
        .unwrap()
        .id;
    let phone = NewDevice {
        name: "Phone".to_owned(),
        ip_address: "192.0.2.7".parse().unwrap(),
        user_agent: Some("Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X)".to_owned()),
    };
    let device = db
        .write(move |tx| {
            Box::pin(async move { insert_device(tx, user, None, 0, phone, &hash_token("p")).await })
        })
        .await
        .unwrap()
        .id;
    let users = Users::new(db, ExtensionPoints::default());

    let kinds = users.device_kinds(&[device, device + 1]).await.unwrap();

    assert_eq!(kinds, [(device, user, DeviceKind::DEVICE_KIND_PHONE)]);
}
