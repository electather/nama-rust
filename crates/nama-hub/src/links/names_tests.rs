use sqlx::PgPool;
use uuid::Uuid;

use super::*;

const ID: Uuid = Uuid::from_u128(0x0193_b6f2_c4d8_7a1e_9f3c_5b2a_1d0e_4f67);

/// [`ID`]'s own name in the zone `zone`.
const OWN: &str = "0193b6f2c4d87a1e9f3c5b2a1d0e4f67.zone";

fn own() -> OwnName {
    OwnName::new(LinkedServerId(ID), "zone")
}

fn lan(addresses: &[&str]) -> Vec<String> {
    addresses
        .iter()
        .map(|&address| address.to_owned())
        .collect()
}

fn ip(ip: &str) -> IpAddr {
    ip.parse().unwrap()
}

fn record(name: &str, address: &str) -> Record {
    Record {
        name: name.to_owned(),
        address: ip(address),
    }
}

#[test]
fn a_servers_own_name_is_its_id_in_hex_under_the_zone() {
    assert_eq!(
        OwnName::new(LinkedServerId(ID), "namadirect.net"),
        OwnName("0193b6f2c4d87a1e9f3c5b2a1d0e4f67.namadirect.net".to_owned())
    );
}

#[test]
fn the_relays_address_is_its_name_under_the_own_name_on_the_relays_port() {
    assert_eq!(
        own().relay_address(8443),
        format!("https://relay.{OWN}:8443")
    );
}

#[test]
fn a_lan_names_address_is_on_that_addresss_port() {
    assert_eq!(
        own().lan_name_address("http://192.168.1.10:7980"),
        Some(format!("https://192-168-1-10.{OWN}:7980"))
    );
    assert_eq!(
        own().lan_name_address("http://10.0.0.5"),
        Some(format!("https://10-0-0-5.{OWN}:80"))
    );
}

#[test]
fn public_host_named_and_ipv6_lan_addresses_get_no_name() {
    for lan in [
        "http://8.8.8.8:7980",
        "http://nama.local:7980",
        "http://[fd00::1]:7980",
    ] {
        assert_eq!(own().lan_name_address(lan), None, "{lan}");
    }
}

#[test]
fn each_private_ipv4_lan_address_gets_a_record_carrying_it_once() {
    let lan = lan(&[
        "http://192.168.1.10:7980",
        "https://192.168.1.10:8443",
        "http://10.0.0.5:7980",
        "http://8.8.8.8:7980",
        "http://nama.local:7980",
        "http://[fd00::1]:7980",
    ]);

    assert_eq!(
        own().records(None, &lan, None),
        [
            record(&format!("10-0-0-5.{OWN}"), "10.0.0.5"),
            record(&format!("192-168-1-10.{OWN}"), "192.168.1.10"),
        ]
    );
}

#[test]
fn the_own_name_points_at_the_public_ip_reached_and_the_relay_name_at_the_relay() {
    let lan = lan(&["http://10.0.0.5:7980"]);

    assert_eq!(
        own().records(Some(ip("2001:db8::7")), &lan, Some(ip("198.51.100.4"))),
        [
            record(OWN, "2001:db8::7"),
            record(&format!("10-0-0-5.{OWN}"), "10.0.0.5"),
            record(&format!("relay.{OWN}"), "198.51.100.4"),
        ]
    );
}

#[test]
fn a_server_is_told_its_own_name_first_reached_or_not() {
    let lan = lan(&["http://10.0.0.5:7980"]);
    let relay = Some(ip("198.51.100.4"));
    let unreached = own().records(None, &lan, relay);
    let reached = own().records(Some(ip("203.0.113.7")), &lan, relay);
    let told = [
        OWN.to_owned(),
        format!("10-0-0-5.{OWN}"),
        format!("relay.{OWN}"),
    ];

    assert_eq!(own().names(&unreached), told);
    assert_eq!(own().names(&reached), told);
}

#[sqlx::test(migrations = false)]
async fn a_hub_without_a_zone_names_no_server_and_still_unlinks(db: PgPool) {
    let names = Names::new(db, None);
    let server = LinkedServerId(ID);
    let lan = lan(&["http://192.168.1.10:7980"]);
    let mut removed = false;

    let own_name = names.own_name(server);
    let told = (names.publish_report(server, Some(ip("203.0.113.7")), &lan, None))
        .await
        .unwrap();
    let challenge = names.publish_challenge(server, Vec::new()).await;
    let unlinking = async {
        removed = true;
        Ok(())
    };
    names.unlink(server, unlinking).await.unwrap();
    names.forget_deleted([server]).await;

    assert_eq!(names.own(server), None);
    assert!(matches!(own_name, Err(Error::NamesOff)), "{own_name:?}");
    assert_eq!(told, Vec::<String>::new());
    assert!(matches!(challenge, Err(Error::NamesOff)), "{challenge:?}");
    assert!(removed);
}
