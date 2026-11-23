//! A Linked Server reporting its Server Addresses, the Hub probing its public port, and an
//! Account listing them, against a real PostgreSQL.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/hub.rs"]
mod hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/linking.rs"]
mod linking;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/mail.rs"]
mod mail;
#[path = "support/refused.rs"]
mod refused;

use connectrpc::ErrorCode;
use hub::TestHub;
use linking::*;
use nama_hub_proto::nama::hub::v1::{
    ListLinkedServersRequest, RefusalReason, SetServerAddressesRequest, SetServerAddressesResponse,
};
use refused::assert_refused;
use sqlx::PgPool;

/// Reports `url` and `lan_addresses` as the Server Addresses of the Server presenting `credential`.
async fn set_addresses(
    hub: &TestHub,
    credential: &str,
    url: &str,
    lan_addresses: &[&str],
) -> Result<(), connectrpc::ConnectError> {
    set_addresses_with_port(hub, credential, url, lan_addresses, 0).await?;
    Ok(())
}

/// Reports `url`, `lan_addresses` and `public_port` as the Server Addresses of the Server
/// presenting `credential`, and answers what the Hub found at the port.
async fn set_addresses_with_port(
    hub: &TestHub,
    credential: &str,
    url: &str,
    lan_addresses: &[&str],
    public_port: u32,
) -> Result<SetServerAddressesResponse, connectrpc::ConnectError> {
    let set = linked_server(hub, credential)
        .set_server_addresses(SetServerAddressesRequest {
            url: url.to_owned(),
            lan_addresses: lan_addresses
                .iter()
                .map(|&address| address.to_owned())
                .collect(),
            public_port,
            ..Default::default()
        })
        .await?;
    Ok(set.into_owned())
}

/// Each Server the Account signed in as `token` lists: its name, its Server URL and its Server
/// Addresses.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn server_addresses(hub: &TestHub, token: &str) -> Vec<(String, String, Vec<String>)> {
    let listed = links(hub, token)
        .list_linked_servers(ListLinkedServersRequest::default())
        .await
        .unwrap()
        .into_owned();
    listed
        .servers
        .into_iter()
        .map(|server| (server.name, server.url, server.addresses))
        .collect()
}

#[sqlx::test(migrations = false)]
async fn an_account_lists_the_server_addresses_its_server_last_reported(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    assert_eq!(
        server_addresses(&hub, &ada).await,
        [("Living room".to_owned(), String::new(), vec![])]
    );
    set_addresses(&hub, &living_room, "", &["http://192.168.1.10:7980"])
        .await
        .unwrap();

    set_addresses(
        &hub,
        &living_room,
        "https://nama.example.com",
        &["http://192.168.1.11:7980"],
    )
    .await
    .unwrap();

    assert_eq!(
        server_addresses(&hub, &ada).await,
        [(
            "Living room".to_owned(),
            "https://nama.example.com".to_owned(),
            vec![
                "https://nama.example.com".to_owned(),
                "http://192.168.1.11:7980".to_owned()
            ]
        )]
    );
}

#[sqlx::test(migrations = false)]
async fn an_account_lists_the_servers_its_users_are_on_and_no_others(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    let cy = create_proven(&hub, &mut inbox, "cy@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    linked(&hub, &ada, "Cabin").await;
    set_addresses(&hub, &living_room, "https://nama.example.com", &[])
        .await
        .unwrap();
    let code = start_user(&hub, &living_room, 7).await.unwrap();
    approve_user(&hub, &bea, &code).await.unwrap();

    assert_eq!(
        server_addresses(&hub, &bea).await,
        [(
            "Living room".to_owned(),
            "https://nama.example.com".to_owned(),
            vec!["https://nama.example.com".to_owned()]
        )]
    );
    assert_eq!(server_names(&hub, &ada).await, ["Living room", "Cabin"]);
    assert_eq!(server_names(&hub, &cy).await, Vec::<String>::new());
}

#[sqlx::test(migrations = false)]
async fn a_server_sets_only_its_own_addresses(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let bea = create_proven(&hub, &mut inbox, "bea@example.com").await;
    linked(&hub, &ada, "Ada's").await;
    let beas = linked(&hub, &bea, "Bea's").await;

    set_addresses(&hub, &beas, "https://bea.example.com", &[])
        .await
        .unwrap();

    assert_eq!(
        server_addresses(&hub, &ada).await,
        [("Ada's".to_owned(), String::new(), vec![])]
    );
}

#[sqlx::test(migrations = false)]
async fn a_server_address_must_be_an_http_url(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    let too_many = ["http://192.168.1.10:7980"; 17];
    let too_long = format!("https://{}.example.com", "a".repeat(2000));

    for (url, lan_addresses) in [
        ("javascript:alert(1)", &[][..]),
        ("", &["192.168.1.10:7980"][..]),
        (too_long.as_str(), &[][..]),
        ("", &too_many[..]),
    ] {
        assert_refused(
            set_addresses(&hub, &living_room, url, lan_addresses).await,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_SERVER_ADDRESS_INVALID,
        );
    }
    let port_refusal = assert_refused(
        set_addresses_with_port(&hub, &living_room, "", &[], 65536).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_SERVER_ADDRESS_INVALID,
    );
    assert_eq!(port_refusal.params["maxPort"], "65535");
    assert_eq!(
        server_addresses(&hub, &ada).await,
        [("Living room".to_owned(), String::new(), vec![])]
    );
}

#[sqlx::test(migrations = false)]
async fn the_hub_lists_the_public_address_it_reached_between_the_server_url_and_the_lan(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    // The test's calls come from 127.0.0.1, so the Hub tries the Server's port there.
    let open = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = open.local_addr().unwrap().port();
    let lan = ["http://192.168.1.10:7980"];

    let reached = set_addresses_with_port(
        &hub,
        &living_room,
        "https://nama.example.com",
        &lan,
        port.into(),
    )
    .await
    .unwrap();
    let listed = server_addresses(&hub, &ada).await;
    drop(open);
    let missed = set_addresses_with_port(
        &hub,
        &living_room,
        "https://nama.example.com",
        &lan,
        port.into(),
    )
    .await
    .unwrap();

    let public = format!("http://127.0.0.1:{port}");
    assert_eq!(
        (reached.public_address.as_str(), reached.reachable),
        (public.as_str(), true)
    );
    assert_eq!(
        listed[0].2,
        [
            "https://nama.example.com",
            public.as_str(),
            "http://192.168.1.10:7980"
        ]
    );
    assert_eq!(
        (missed.public_address.as_str(), missed.reachable),
        (public.as_str(), false)
    );
    assert_eq!(
        server_addresses(&hub, &ada).await[0].2,
        ["https://nama.example.com", "http://192.168.1.10:7980"]
    );
}

#[sqlx::test(migrations = false)]
async fn the_hub_stores_a_new_server_url_at_once_and_keeps_the_public_address_while_it_probes(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    // The test's calls come from 127.0.0.1, so the Hub tries the Server's ports there.
    let open = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let reached = open.local_addr().unwrap().port();
    set_addresses_with_port(
        &hub,
        &living_room,
        "https://old.example.com",
        &[],
        reached.into(),
    )
    .await
    .unwrap();
    // Linux drops a SYN to a listener whose accept queue is full, so the Hub's connection waits
    // out its whole probe: a backlog of 0 queues one connection, which `_queued` takes.
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let full = socket.listen(0).unwrap();
    let port = full.local_addr().unwrap().port();
    let _queued = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();

    let set = set_addresses_with_port(
        &hub,
        &living_room,
        "https://nama.example.com",
        &[],
        port.into(),
    );
    let stored = async {
        loop {
            let (_, url, addresses) = server_addresses(&hub, &ada).await.swap_remove(0);
            if url == "https://nama.example.com" {
                return addresses;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    };
    // Well inside the Hub's 5-second probe, so storing the Server URL only after it fails here.
    let stored = tokio::time::timeout(std::time::Duration::from_secs(2), stored);

    let listed = tokio::select! {
        listed = stored => listed.expect("the Hub stored the Server URL only after its probe"),
        answer = set => panic!("the probe ended at once, so it held nothing: {answer:?}"),
    };
    assert_eq!(
        listed,
        [
            "https://nama.example.com".to_owned(),
            format!("http://127.0.0.1:{reached}")
        ]
    );
}
