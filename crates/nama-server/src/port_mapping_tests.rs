use std::sync::{Arc, Mutex};

use axum::{http::HeaderMap, routing};

use super::*;

const DESCRIPTION: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <device>
    <deviceType>urn:schemas-upnp-org:device:InternetGatewayDevice:1</deviceType>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType>
        <controlURL>/ctl/L3F</controlURL>
      </service>
    </serviceList>
    <deviceList><device><deviceList><device>
      <serviceList>
        <service>
          <serviceType>urn:schemas-upnp-org:service:WANIPConnection:2</serviceType>
          <controlURL> /ctl/IPConn </controlURL>
        </service>
      </serviceList>
    </device></deviceList></device></deviceList>
  </device>
</root>"#;

#[test]
fn a_routers_search_answer_names_where_its_description_is() {
    let answer = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\n\
                  Location: http://192.168.1.1:5000/rootDesc.xml\r\n\r\n";

    assert_eq!(
        location(answer),
        Some("http://192.168.1.1:5000/rootDesc.xml")
    );
    assert_eq!(location("HTTP/1.1 200 OK\r\n\r\n"), None);
}

#[test]
fn the_wan_connection_is_found_anywhere_in_the_routers_description() {
    let at = Url::parse("http://192.168.1.1:5000/rootDesc.xml").unwrap();

    let (control, service) = wan_connection(DESCRIPTION, &at).unwrap();

    assert_eq!(control.as_str(), "http://192.168.1.1:5000/ctl/IPConn");
    assert_eq!(service, "urn:schemas-upnp-org:service:WANIPConnection:2");
    let odd_type = DESCRIPTION.replace("WANIPConnection:2", "WANIPConnection:2\"");
    assert_eq!(wan_connection(&odd_type, &at), None);
    assert_eq!(wan_connection("not xml", &at), None);
}

#[tokio::test]
async fn upnp_asks_the_router_to_forward_the_servers_port_to_it() {
    let asked: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let heard = Arc::clone(&asked);
    let router = axum::Router::new()
        .route("/rootDesc.xml", routing::get(async || DESCRIPTION))
        .route(
            "/ctl/IPConn",
            routing::post(async move |headers: HeaderMap, body: String| {
                let action = headers["soapaction"].to_str().unwrap().to_owned();
                heard.lock().unwrap().push((action, body));
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let description = Url::parse(&format!(
        "http://{}/rootDesc.xml",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let serving = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    let opened = add_port_mapping(
        &description,
        SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), 7980),
    )
    .await;
    serving.abort();

    assert_eq!(opened.unwrap(), 7980);
    let asked = asked.lock().unwrap();
    let [(action, body)] = asked.as_slice() else {
        panic!("{asked:?}");
    };
    assert_eq!(
        action,
        "\"urn:schemas-upnp-org:service:WANIPConnection:2#AddPortMapping\""
    );
    for told in [
        "<NewExternalPort>7980</NewExternalPort>",
        "<NewProtocol>TCP</NewProtocol>",
        "<NewInternalPort>7980</NewInternalPort>",
        "<NewInternalClient>192.168.1.10</NewInternalClient>",
        "<NewLeaseDuration>3600</NewLeaseDuration>",
    ] {
        assert!(body.contains(told), "{body}");
    }
}

#[test]
fn the_default_gateway_is_read_from_linuxs_routing_table() {
    let routes = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\n\
                  eth0\t0001A8C0\t00000000\t0001\t0\t0\t0\t00FFFFFF\n\
                  eth0\t00000000\t0101A8C0\t0003\t0\t0\t0\t00000000\n";
    let expected = if cfg!(target_endian = "little") {
        Ipv4Addr::new(192, 168, 1, 1)
    } else {
        Ipv4Addr::new(1, 1, 168, 192)
    };

    assert_eq!(default_gateway(routes), Some(expected));
    assert_eq!(default_gateway("Iface\tDestination\tGateway\n"), None);
}

#[test]
fn nat_pmp_asks_for_the_servers_tcp_port_for_the_lease() {
    assert_eq!(
        nat_pmp_request(7980),
        [0, 2, 0, 0, 0x1f, 0x2c, 0x1f, 0x2c, 0, 0, 0x0e, 0x10]
    );
}

#[test]
fn a_nat_pmp_answer_gives_the_public_port_only_on_success() {
    let mapped = [
        0, 130, 0, 0, 0, 0, 0, 9, 0x1f, 0x2c, 0x1f, 0x2d, 0, 0, 0x0e, 0x10,
    ];
    let mut refused = mapped;
    refused[3] = 2;

    assert_eq!(nat_pmp_mapped(&mapped), Some(7981));
    assert_eq!(nat_pmp_mapped(&refused), None);
    assert_eq!(nat_pmp_mapped(&mapped[..4]), None);
}
