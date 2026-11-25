use super::*;

#[test]
fn the_same_addresses_go_to_the_hub_again_once_its_check_is_old() {
    let at = Instant::now();
    let addresses = ServerAddresses {
        public_port: Some(7980),
        ..ServerAddresses::default()
    };
    let reported = Reported {
        credential: "link".to_owned(),
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

#[test]
fn a_sign_in_token_passes_only_for_this_server_and_one_without_its_id_learns_it() {
    assert_eq!(learns_audience(Some("server"), "server"), Ok(false));
    learns_audience(Some("server"), "another server").unwrap_err();
    assert_eq!(learns_audience(None, "server"), Ok(true));
    learns_audience(None, "").unwrap_err();
}
