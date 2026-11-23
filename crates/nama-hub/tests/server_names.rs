//! A Linked Server's names in the Server-names zone, which the Hub writes as the Server reports
//! its addresses and removes as it Unlinks (ADR 0104), against a real PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/linking.rs"]
mod linking;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;

use connectrpc::ConnectError;
use hub::{TestHub, ZONE};
use linking::*;
use nama_hub::dns::Record;
use nama_hub_proto::nama::hub::v1::{
    ListLinkedServersRequest, SetServerAddressesRequest, UnlinkServerRequest,
};
use sqlx::PgPool;

/// Reports `lan_addresses` and `public_port` as the Server Addresses of the Server presenting
/// `credential`, and answers the names the Hub gave it.
async fn names_after_reporting(
    hub: &TestHub,
    credential: &str,
    lan_addresses: &[&str],
    public_port: u16,
) -> Result<Vec<String>, ConnectError> {
    let set = linked_server(hub, credential)
        .set_server_addresses(SetServerAddressesRequest {
            lan_addresses: lan_addresses
                .iter()
                .map(|&address| address.to_owned())
                .collect(),
            public_port: public_port.into(),
            ..Default::default()
        })
        .await?;
    Ok(set.into_owned().names)
}

async fn unlink(hub: &TestHub, credential: &str) -> Result<(), ConnectError> {
    linked_server(hub, credential)
        .unlink_server(UnlinkServerRequest::default())
        .await?;
    Ok(())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn record(name: &str, address: &str) -> Record {
    Record {
        name: name.to_owned(),
        address: address.parse().unwrap(),
    }
}

#[sqlx::test(migrations = false)]
async fn a_server_gets_its_own_name_and_one_for_each_private_lan_ip(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    // The test's calls come from 127.0.0.1, so the Hub tries the Server's port there.
    let open = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = open.local_addr().unwrap().port();

    let names = names_after_reporting(
        &hub,
        &living_room,
        &[
            "http://192.168.1.10:7980",
            "http://10.0.0.5:7980",
            "http://203.0.113.9:7980",
            "http://nama.local:7980",
        ],
        port,
    )
    .await
    .unwrap();

    let own = &names[0];
    let label = own.strip_suffix(&format!(".{ZONE}")).unwrap();
    assert!(
        label.len() == 32 && label.bytes().all(|c| c.is_ascii_hexdigit()),
        "{own}"
    );
    assert_eq!(
        names[1..],
        [format!("10-0-0-5.{own}"), format!("192-168-1-10.{own}")]
    );
    let mut wanted = [
        record(own, "127.0.0.1"),
        record(&format!("10-0-0-5.{own}"), "10.0.0.5"),
        record(&format!("192-168-1-10.{own}"), "192.168.1.10"),
    ];
    wanted.sort();
    assert_eq!(hub.zone(), wanted);
}

#[sqlx::test(migrations = false)]
async fn a_servers_own_name_stays_as_its_addresses_change_and_resolves_only_while_reached(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    let first = names_after_reporting(&hub, &living_room, &["http://192.168.1.10:7980"], 0)
        .await
        .unwrap();

    let then = names_after_reporting(&hub, &living_room, &["http://192.168.1.11:7980"], 0)
        .await
        .unwrap();

    let own = &first[0];
    assert_eq!(then, [own.clone(), format!("192-168-1-11.{own}")]);
    assert_eq!(
        hub.zone(),
        [record(&format!("192-168-1-11.{own}"), "192.168.1.11")]
    );
}

#[sqlx::test(migrations = false)]
async fn unlinking_a_server_removes_its_names_and_no_others(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    let cabin = linked(&hub, &ada, "Cabin").await;
    names_after_reporting(&hub, &living_room, &["http://192.168.1.10:7980"], 0)
        .await
        .unwrap();
    let cabins = names_after_reporting(&hub, &cabin, &["http://10.0.0.5:7980"], 0)
        .await
        .unwrap();

    unlink(&hub, &living_room).await.unwrap();

    assert_eq!(
        hub.zone(),
        [record(&format!("10-0-0-5.{}", cabins[0]), "10.0.0.5")]
    );
    assert_eq!(server_names(&hub, &ada).await, ["Cabin"]);
}

#[sqlx::test(migrations = false)]
async fn a_server_stays_linked_while_its_names_cant_be_removed(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    names_after_reporting(&hub, &living_room, &["http://192.168.1.10:7980"], 0)
        .await
        .unwrap();
    hub.set_zone_down(true);

    unlink(&hub, &living_room).await.unwrap_err();

    assert_eq!(server_names(&hub, &ada).await, ["Living room"]);
    hub.set_zone_down(false);
    unlink(&hub, &living_room).await.unwrap();
    assert_eq!(hub.zone(), []);
}

#[sqlx::test(migrations = false)]
async fn a_report_the_dns_provider_fails_still_lists_the_public_address_reached(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    // The test's calls come from 127.0.0.1, so the Hub tries the Server's port there.
    let open = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = open.local_addr().unwrap().port();
    hub.set_zone_down(true);

    names_after_reporting(&hub, &living_room, &[], port)
        .await
        .unwrap_err();

    let listed = links(&hub, &ada)
        .list_linked_servers(ListLinkedServersRequest::default())
        .await
        .unwrap()
        .into_owned();
    assert_eq!(
        listed.servers[0].addresses,
        [format!("http://127.0.0.1:{port}")]
    );
}
