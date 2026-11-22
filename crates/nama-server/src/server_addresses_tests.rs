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
