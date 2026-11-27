//! A Linked Server reporting its Server Addresses, the Hub probing its public port, and an
//! Account listing them, against a real PostgreSQL.

use crate::support::hub::TestHub;
use crate::support::linking::*;
use crate::support::mail::fake_mail;
use crate::support::refused::assert_refused;
use connectrpc::ErrorCode;
use nama_hub_proto::nama::hub::v1::{
    ListLinkedServersRequest, RefusalReason, SetServerAddressesRequest, SetServerAddressesResponse,
    StartServerSignInRequest,
};
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

/// Starts a sign-in to the Server presenting `credential`, returning to `return_url`.
async fn start_sign_in(
    hub: &TestHub,
    credential: &str,
    return_url: &str,
) -> Result<(), connectrpc::ConnectError> {
    linked_server(hub, credential)
        .start_server_sign_in(StartServerSignInRequest {
            return_url: return_url.to_owned(),
            ..Default::default()
        })
        .await?;
    Ok(())
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
async fn a_server_address_from_a_server_url_ending_in_a_slash_has_none(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    set_addresses(&hub, &living_room, "https://example.com/nama/", &[])
        .await
        .unwrap();

    assert_eq!(
        server_addresses(&hub, &ada).await,
        [(
            "Living room".to_owned(),
            "https://example.com/nama/".to_owned(),
            vec!["https://example.com/nama".to_owned()]
        )]
    );
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
async fn a_server_url_a_browser_opens_is_admitted_listed_as_sent_and_returned_to(db: PgPool) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;

    set_addresses(&hub, &living_room, "https://bücher.example", &[])
        .await
        .unwrap();

    assert_eq!(
        server_addresses(&hub, &ada).await,
        [(
            "Living room".to_owned(),
            "https://bücher.example".to_owned(),
            vec!["https://bücher.example".to_owned()]
        )]
    );
    start_sign_in(&hub, &living_room, "https://BÜCHER.example/sign-in")
        .await
        .unwrap();
    start_sign_in(&hub, &living_room, "https://xn--bcher-kva.example/sign-in")
        .await
        .unwrap();
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
    let (mail, mut inbox) = fake_mail();
    let (hub, mut probes) = TestHub::start_holding_probes(db, Some(mail)).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    let first = set_addresses_with_port(&hub, &living_room, "https://old.example.com", &[], 7980);
    let answered = async { probes.recv().await.unwrap().send(true).unwrap() };
    tokio::join!(first, answered).0.unwrap();

    let set = set_addresses_with_port(&hub, &living_room, "https://nama.example.com", &[], 7980);
    let listed_while_probing = async {
        let held = probes.recv().await.unwrap();
        let listed = server_addresses(&hub, &ada).await.swap_remove(0);
        held.send(true).unwrap();
        listed
    };
    let (set, (_, url, addresses)) = tokio::join!(set, listed_while_probing);

    set.unwrap();
    assert_eq!(url, "https://nama.example.com");
    assert_eq!(
        addresses,
        ["https://nama.example.com", "http://127.0.0.1:7980"]
    );
}

#[sqlx::test(migrations = false)]
async fn a_sign_in_never_returns_to_the_public_address_as_the_server_cant_know_its_public_ip(
    db: PgPool,
) {
    let (hub, mut inbox) = hub_with_mail(db).await;
    let ada = create_proven(&hub, &mut inbox, "ada@example.com").await;
    let living_room = linked(&hub, &ada, "Living room").await;
    // The test's calls come from 127.0.0.1, so the Hub tries the Server's port there.
    let open = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = open.local_addr().unwrap().port();
    let public = format!("http://127.0.0.1:{port}");
    set_addresses_with_port(
        &hub,
        &living_room,
        "https://nama.example.com",
        &[],
        port.into(),
    )
    .await
    .unwrap();
    assert!(server_addresses(&hub, &ada).await[0].2.contains(&public));

    assert_refused(
        start_sign_in(&hub, &living_room, &format!("{public}/sign-in")).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_URL_INVALID,
    );
}
