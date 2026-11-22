use super::*;

#[test]
fn the_same_addresses_go_to_the_hub_again_once_its_check_is_old() {
    let at = Instant::now();
    let addresses = ServerAddresses {
        public_port: Some(7980),
        ..ServerAddresses::default()
    };
    let reported = Reported {
        link: "link".to_owned(),
        addresses: addresses.clone(),
        at,
    };
    let moved = ServerAddresses {
        public_port: Some(7981),
        ..ServerAddresses::default()
    };

    assert!(reported.covers("link", &addresses, at + RECHECK_EVERY / 2));
    assert!(!reported.covers("link", &addresses, at + RECHECK_EVERY));
    assert!(!reported.covers("link", &moved, at));
    assert!(!reported.covers("another link", &addresses, at));
}
