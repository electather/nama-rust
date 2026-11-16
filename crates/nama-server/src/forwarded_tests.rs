use axum::http::{HeaderMap, HeaderValue};

use super::request_source;

#[test]
fn a_forged_chain_cant_name_a_client_past_the_trusted_hops() {
    let trusted = ["10.0.0.0/8".parse().unwrap()];
    let source = |lines: &[&str]| {
        let mut headers = HeaderMap::new();
        for line in lines {
            headers.append("x-forwarded-for", HeaderValue::from_str(line).unwrap());
        }
        request_source("10.0.0.1".parse().unwrap(), &headers, &trusted).client_ip
    };

    let cases: [(&[&str], &str); 5] = [
        (&["1.2.3.4, 198.51.100.7, 10.0.0.5"], "198.51.100.7"),
        (
            &["1.2.3.4, ::ffff:198.51.100.7, ::ffff:10.0.0.5"],
            "198.51.100.7",
        ),
        (&["1.2.3.4", "198.51.100.7", "10.0.0.5"], "198.51.100.7"),
        (&["1.2.3.4", "198.51.100.7:1234, 10.0.0.5"], "10.0.0.5"),
        (&[""], "10.0.0.1"),
    ];
    for (lines, client) in cases {
        assert_eq!(
            source(lines),
            client.parse::<std::net::IpAddr>().unwrap(),
            "{lines:?}"
        );
    }
}

#[test]
fn an_ipv4_peer_mapped_into_ipv6_is_its_ipv4_address() {
    let trusted = ["10.0.0.0/8".parse().unwrap()];
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.7"));
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));

    let untrusted = request_source("::ffff:203.0.113.9".parse().unwrap(), &headers, &trusted);
    let proxied = request_source("::ffff:10.0.0.1".parse().unwrap(), &headers, &trusted);

    assert_eq!(
        untrusted.client_ip,
        "203.0.113.9".parse::<std::net::IpAddr>().unwrap()
    );
    assert!(!untrusted.https);
    assert_eq!(
        proxied.client_ip,
        "198.51.100.7".parse::<std::net::IpAddr>().unwrap()
    );
    assert!(proxied.https);
}
