use super::*;

#[test]
fn the_lan_address_is_the_one_the_server_listens_on() {
    assert_eq!(
        lan_address("192.168.1.10:7980".parse().unwrap()).as_deref(),
        Some("http://192.168.1.10:7980")
    );
    assert_eq!(
        lan_address("[fd00::10]:7980".parse().unwrap()).as_deref(),
        Some("http://[fd00::10]:7980")
    );
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
        address
            .as_ref()
            .is_none_or(|address| address.starts_with("http://")
                && address.ends_with(":7980")
                && !address.contains("0.0.0.0")),
        "{address:?}"
    );
}
