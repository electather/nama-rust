use std::net::IpAddr;

use uuid::Uuid;

use super::*;

const ID: Uuid = Uuid::from_u128(0x0193_b6f2_c4d8_7a1e_9f3c_5b2a_1d0e_4f67);

fn record(name: &str, address: &str) -> Record {
    Record {
        name: name.to_owned(),
        address: address.parse().unwrap(),
    }
}

#[test]
fn a_servers_own_name_is_its_id_in_hex_under_the_zone() {
    assert_eq!(
        own_name(LinkedServerId(ID), "namadirect.net"),
        "0193b6f2c4d87a1e9f3c5b2a1d0e4f67.namadirect.net"
    );
}

#[test]
fn each_private_ipv4_lan_address_gets_a_name_carrying_it_once() {
    let lan = [
        "http://192.168.1.10:7980",
        "https://192.168.1.10:8443",
        "http://10.0.0.5:7980",
        "http://8.8.8.8:7980",
        "http://nama.local:7980",
        "http://[fd00::1]:7980",
    ]
    .map(str::to_owned);

    assert_eq!(
        records("own.zone", None, &lan, None),
        [
            record("10-0-0-5.own.zone", "10.0.0.5"),
            record("192-168-1-10.own.zone", "192.168.1.10"),
        ]
    );
}

#[test]
fn the_own_name_points_at_the_public_address_the_hub_reached() {
    let public: IpAddr = "2001:db8::7".parse().unwrap();

    assert_eq!(
        records(
            "own.zone",
            Some(public),
            &["http://10.0.0.5:7980".to_owned()],
            None,
        ),
        [
            record("own.zone", "2001:db8::7"),
            record("10-0-0-5.own.zone", "10.0.0.5"),
        ]
    );
}

#[test]
fn the_relay_name_under_the_own_name_points_at_the_relay() {
    let relay: IpAddr = "198.51.100.4".parse().unwrap();

    assert_eq!(
        records(
            "own.zone",
            None,
            &["http://10.0.0.5:7980".to_owned()],
            Some(relay)
        ),
        [
            record("10-0-0-5.own.zone", "10.0.0.5"),
            record("relay.own.zone", "198.51.100.4"),
        ]
    );
}
