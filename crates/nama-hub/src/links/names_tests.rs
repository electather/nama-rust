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
