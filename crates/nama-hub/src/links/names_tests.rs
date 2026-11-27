use sqlx::PgPool;
use uuid::Uuid;

use super::*;

const ID: Uuid = Uuid::from_u128(0x0193_b6f2_c4d8_7a1e_9f3c_5b2a_1d0e_4f67);

#[test]
fn a_servers_own_name_is_its_id_in_hex_under_the_zone() {
    assert_eq!(
        own_name(LinkedServerId(ID), "namadirect.net"),
        "0193b6f2c4d87a1e9f3c5b2a1d0e4f67.namadirect.net"
    );
}

#[sqlx::test(migrations = false)]
async fn a_hub_without_a_zone_names_no_server_and_still_unlinks(db: PgPool) {
    let names = Names::new(db, None);
    let server = LinkedServerId(ID);
    let lan = ["http://192.168.1.10:7980".to_owned()];
    let addresses = ServerAddresses::new(None, "", &lan, None, None);
    let mut removed = false;

    let own = names.own_name(server);
    let told = names.publish_report(server, &addresses).await.unwrap();
    let challenge = names.publish_challenge(server, Vec::new()).await;
    let unlinking = async {
        removed = true;
        Ok(())
    };
    names.unlink(server, unlinking).await.unwrap();
    names.forget_deleted(vec![server]).await;

    assert!(matches!(own, Err(Error::NamesOff)), "{own:?}");
    assert_eq!(told, Vec::<String>::new());
    assert!(matches!(challenge, Err(Error::NamesOff)), "{challenge:?}");
    assert!(removed);
}
