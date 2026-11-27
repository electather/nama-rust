use super::*;

#[test]
fn the_lan_address_is_the_one_the_server_listens_on() {
    for listening in ["192.168.1.10:7980", "[fd00::10]:7980"] {
        let listening = listening.parse().unwrap();
        assert_eq!(lan_address(listening), Some(listening));
    }
}

#[test]
fn a_server_listening_on_loopback_has_no_lan_address() {
    assert_eq!(lan_address("127.0.0.1:7980".parse().unwrap()), None);
    assert_eq!(lan_address("[::1]:7980".parse().unwrap()), None);
}

#[test]
fn a_server_listening_on_every_address_has_its_default_routes_or_none() {
    let address = lan_address("0.0.0.0:7980".parse().unwrap());

    // Whether the machine running the test has a default route is its own affair.
    assert!(
        address.is_none_or(|address| address.port() == 7980 && !address.ip().is_unspecified()),
        "{address:?}"
    );
}

#[test]
fn the_server_url_is_sent_in_ascii_as_typed_otherwise() {
    assert_eq!(
        as_sent("https://médias.example.com"),
        "https://xn--mdias-bsa.example.com/"
    );
    assert_eq!(
        as_sent("https://example.com/my media"),
        "https://example.com/my%20media"
    );
    assert_eq!(as_sent(""), "");
}

#[test]
fn a_reported_address_matches_the_lan_one_by_scheme_host_and_port() {
    // No Server URL set, so the LAN address is the one reported.
    let url = ServerUrl::default();
    let reported = |address| {
        is_reported(
            address,
            &url,
            "192.168.1.2:7980".parse().unwrap(),
            None,
            &[],
        )
    };

    assert!(reported("HTTP://192.168.1.2:7980/sign-in"));
    assert!(!reported("https://192.168.1.2:7980/sign-in"));
    assert!(!reported("http://192.168.1.2/sign-in"));
    assert!(!reported("http://localhost:7980/sign-in"));
    assert!(!reported("/sign-in"));
}

#[test]
fn a_reported_address_may_be_at_the_servers_lan_name_or_relay_and_never_another_servers() {
    let own = "0193b6f2c4d87a1e9f3c5b2a1d0e4f67.namadirect.net";
    let names = [
        own.to_owned(),
        format!("192-168-1-2.{own}"),
        format!("relay.{own}"),
    ];
    let url = ServerUrl::default();
    let reported = |address: &str| {
        is_reported(
            address,
            &url,
            "192.168.1.2:7980".parse().unwrap(),
            Some("198.51.100.4:443".parse().unwrap()),
            &names,
        )
    };

    assert!(reported(&format!("https://192-168-1-2.{own}:7980/sign-in")));
    assert!(reported(&format!("https://relay.{own}/sign-in")));
    assert!(!reported(&format!(
        "https://192-168-1-2.{own}:8443/sign-in"
    )));
    assert!(!reported(&format!("https://relay.{own}:7980/sign-in")));
    assert!(!reported(&format!("https://{own}:7980/sign-in")));
    assert!(!reported(
        "https://192-168-1-2.0193b6f2c4d87a1e9f3c5b2a1d0e4f68.namadirect.net:7980/sign-in"
    ));
}
