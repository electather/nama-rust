use uuid::Uuid;

use super::*;

const ID: Uuid = Uuid::from_u128(0x0193_b6f2_c4d8_7a1e_9f3c_5b2a_1d0e_4f67);

/// [`ID`]'s own name in the zone `zone`.
const OWN: &str = "0193b6f2c4d87a1e9f3c5b2a1d0e4f67.zone";

const RELAY: &str = "198.51.100.4:443";

/// The facts of one report: the Server URL, the LAN addresses, the public IP and port reached and
/// the Relay.
struct Report {
    url: &'static str,
    lan: Vec<String>,
    public: Option<&'static str>,
    relay: Option<&'static str>,
}

const NOTHING: Report = Report {
    url: "",
    lan: Vec::new(),
    public: None,
    relay: None,
};

impl Report {
    /// The Server Addresses this report gives, named under [`OWN`] if `named`.
    fn addresses(&self, named: bool) -> ServerAddresses<'_> {
        ServerAddresses::new(
            named.then(|| OwnName::new(LinkedServerId(ID), "zone")),
            self.url,
            &self.lan,
            self.public.map(|public| public.parse().unwrap()),
            self.relay.map(|relay| relay.parse().unwrap()),
        )
    }
}

fn lan(addresses: &[&str]) -> Vec<String> {
    addresses
        .iter()
        .map(|&address| address.to_owned())
        .collect()
}

const ADDRESSES: [&str; 2] = ["https://nama.example", "http://192.168.1.2:7980"];

#[test]
fn a_server_address_is_an_http_or_https_url() {
    assert!(is_address("https://nama.example.com"));
    assert!(is_address("https://nama.example.com/nama"));
    assert!(is_address("http://192.168.1.10:7980"));
    assert!(is_address("http://[fd00::10]:7980"));
    assert!(!is_address("javascript:alert(1)"));
    assert!(!is_address("data:text/html,hi"));
    assert!(!is_address("192.168.1.10:7980"));
    assert!(!is_address("/relative"));
    assert!(!is_address(""));
    assert!(!is_address("https://"));
    assert!(!is_address(&format!(
        "https://{}.example.com",
        "a".repeat(ADDRESS_LENGTH)
    )));
}

#[test]
fn a_server_address_a_browser_opens_is_one_the_hub_admits() {
    assert!(is_address("https://bücher.example"));
    assert!(is_address("https://example.com/my nama"));
}

#[test]
fn an_address_without_slashes_after_its_scheme_is_refused_as_a_hub_page_reads_it_as_a_path() {
    assert!(
        Url::parse("https:nama.example").is_ok_and(|url| url.host_str() == Some("nama.example"))
    );
    assert!(!is_address("https:nama.example"));
    assert!(!is_address("https:/nama.example"));
    assert!(!is_on_server("https:nama.example/sign-in", ADDRESSES));
}

#[test]
fn a_return_address_is_on_a_server_address_by_scheme_host_and_port() {
    assert!(is_on_server("https://nama.example/sign-in?a=b", ADDRESSES));
    assert!(is_on_server("https://NAMA.example:443/sign-in", ADDRESSES));
    assert!(is_on_server("HTTP://192.168.1.2:7980/sign-in", ADDRESSES));
    assert!(!is_on_server("https://evil.example/sign-in", ADDRESSES));
    assert!(!is_on_server("http://nama.example/sign-in", ADDRESSES));
    assert!(!is_on_server("http://192.168.1.2/sign-in", ADDRESSES));
    assert!(!is_on_server(
        "https://nama.example.evil.example/",
        ADDRESSES
    ));
    assert!(!is_on_server(
        "https://nama.example@evil.example/",
        ADDRESSES
    ));
    assert!(!is_on_server("blob:https://nama.example/1", ADDRESSES));
    assert!(!is_on_server("javascript://%0aalert(1)", ADDRESSES));
    assert!(!is_on_server("/sign-in", ADDRESSES));
    assert!(!is_on_server("https://nama.example/sign-in", []));
    assert!(!is_on_server(
        &format!("https://nama.example/{}", "a".repeat(RETURN_ADDRESS_LENGTH)),
        ADDRESSES
    ));
}

#[test]
fn a_return_address_on_an_internationalised_host_matches_however_it_is_spelled() {
    assert!(is_on_server(
        "https://xn--bcher-kva.example/sign-in",
        ["https://bücher.example"]
    ));
    assert!(is_on_server(
        "https://BÜCHER.example/sign-in",
        ["https://xn--bcher-kva.example"]
    ));
}

#[test]
fn the_list_goes_server_url_public_relay_then_lan() {
    let report = Report {
        url: "https://nama.example/",
        lan: lan(&["http://192.168.1.10:7980", "http://nama.local:7980"]),
        public: Some("[2001:db8::7]:7980"),
        relay: Some(RELAY),
    };

    assert_eq!(
        report.addresses(true).listed(),
        [
            "https://nama.example".to_owned(),
            "http://[2001:db8::7]:7980".to_owned(),
            format!("https://relay.{OWN}:443"),
            "http://192.168.1.10:7980".to_owned(),
            "http://nama.local:7980".to_owned(),
        ]
    );
}

#[test]
fn a_server_url_at_the_relays_origin_lists_it_once() {
    // As a Server behind CGNAT sends its Relay's address as its Server URL, through WHATWG's
    // parser: ending in `/`, port 443 left out (ADR 0057).
    let default_port = Report {
        url: "https://relay.0193b6f2c4d87a1e9f3c5b2a1d0e4f67.zone/",
        relay: Some(RELAY),
        ..NOTHING
    };
    let other_port = Report {
        url: "https://relay.0193b6f2c4d87a1e9f3c5b2a1d0e4f67.zone:8443/",
        relay: Some("198.51.100.4:8443"),
        ..NOTHING
    };

    assert_eq!(
        default_port.addresses(true).listed(),
        [format!("https://relay.{OWN}")]
    );
    assert_eq!(
        other_port.addresses(true).listed(),
        [format!("https://relay.{OWN}:8443")]
    );
}

#[test]
fn a_relay_has_no_address_while_the_hub_names_no_server() {
    let report = Report {
        relay: Some(RELAY),
        ..NOTHING
    };

    assert_eq!(report.addresses(false).listed(), Vec::<String>::new());
    assert_eq!(
        report.addresses(true).listed(),
        [format!("https://relay.{OWN}:443")]
    );
    // A Server URL at the Relay's origin is kept, as sent, all the same.
    let at_relays_origin = Report {
        url: "https://relay.0193b6f2c4d87a1e9f3c5b2a1d0e4f67.zone/",
        ..report
    };
    assert_eq!(
        at_relays_origin.addresses(false).listed(),
        [format!("https://relay.{OWN}")]
    );
}

#[test]
fn a_lan_address_at_an_earlier_addresss_origin_is_listed_once() {
    let at_server_url = Report {
        url: "http://192.168.1.11:7980/",
        lan: lan(&[
            "http://192.168.1.10:7980",
            "http://192.168.1.11:7980",
            "HTTP://192.168.1.10:7980/",
        ]),
        ..NOTHING
    };
    let internationalised = Report {
        url: "https://bücher.example",
        lan: lan(&["https://xn--bcher-kva.example:443"]),
        ..NOTHING
    };

    assert_eq!(
        at_server_url.addresses(true).listed(),
        ["http://192.168.1.11:7980", "http://192.168.1.10:7980"]
    );
    assert_eq!(
        internationalised.addresses(true).listed(),
        ["https://bücher.example"]
    );
}

#[test]
fn a_sign_in_returns_to_the_server_url_lan_addresses_their_names_and_the_relay_only() {
    let report = Report {
        url: "https://nama.example/nama",
        lan: lan(&[
            "http://192.168.1.10:7980",
            "http://10.0.0.5",
            "http://8.8.8.8:7980",
            "http://nama.local:7980",
        ]),
        public: Some("203.0.113.7:7980"),
        relay: Some("198.51.100.4:8443"),
    };
    let returns = |url: &str| report.addresses(true).is_return_address(url);

    assert!(returns("https://nama.example/sign-in"));
    assert!(returns("http://192.168.1.10:7980/sign-in"));
    assert!(returns("http://nama.local:7980/sign-in"));
    assert!(returns(&format!("https://192-168-1-10.{OWN}:7980/sign-in")));
    assert!(returns(&format!("https://10-0-0-5.{OWN}:80/sign-in")));
    assert!(returns(&format!("https://relay.{OWN}:8443/sign-in")));
    assert!(!returns(&format!("https://10-0-0-5.{OWN}/sign-in")));
    assert!(!returns(&format!("https://8-8-8-8.{OWN}:7980/sign-in")));
    assert!(!returns(&format!("https://{OWN}/sign-in")));
    assert!(!returns("http://203.0.113.7:7980/sign-in"));
    assert!(!returns("javascript://nama.example/%0aalert(1)"));
}

#[test]
fn a_hub_naming_no_server_returns_only_to_the_addresses_sent() {
    let report = Report {
        url: "https://nama.example",
        lan: lan(&["http://192.168.1.10:7980"]),
        relay: Some(RELAY),
        ..NOTHING
    };
    let none = report.addresses(false);

    assert!(none.is_return_address("https://nama.example/sign-in"));
    assert!(none.is_return_address("http://192.168.1.10:7980/sign-in"));
    assert!(!none.is_return_address(&format!("https://192-168-1-10.{OWN}:7980/")));
    assert!(!none.is_return_address(&format!("https://relay.{OWN}/")));
}

/// A Server URL at the Relay's origin, as a Server behind CGNAT sends it.
const AT_RELAY: &str = "https://relay.0193b6f2c4d87a1e9f3c5b2a1d0e4f67.zone/";

/// A Server URL at the origin of its public LAN address.
const AT_LAN: &str = "http://8.8.8.8:7980/";

#[test]
fn each_combination_lists_each_origin_once_and_returns_where_it_lists() {
    let relay_origin = format!("https://relay.{OWN}");
    let relay_address = format!("https://relay.{OWN}:443");
    let (public, private, public_lan, hostname) = (
        "http://[2001:db8::7]:7980",
        "http://192.168.1.10:7980",
        "http://8.8.8.8:7980",
        "http://nama.local:7980",
    );
    let combinations = [AT_RELAY, AT_LAN].into_iter().flat_map(|url| {
        [(true, true), (true, false), (false, true), (false, false)]
            .map(|(named, relay)| (named, relay, url))
    });

    for (named, relay, url) in combinations {
        let listed = match (url, named && relay) {
            (AT_RELAY, _) => vec![relay_origin.as_str(), public, private, public_lan, hostname],
            (_, true) => vec![
                public_lan,
                public,
                relay_address.as_str(),
                private,
                hostname,
            ],
            (_, false) => vec![public_lan, public, private, hostname],
        };
        let report = Report {
            url,
            lan: lan(&[private, public_lan, hostname]),
            public: Some("[2001:db8::7]:7980"),
            relay: relay.then_some(RELAY),
        };
        let addresses = report.addresses(named);
        let case = format!("named {named}, Relay {relay}, Server URL {url}");
        let returns = |return_url: &str| addresses.is_return_address(return_url);

        assert_eq!(addresses.listed(), listed, "{case}");
        assert_eq!(
            returns(&format!("{relay_origin}/sign-in")),
            url == AT_RELAY || (named && relay),
            "{case}"
        );
        assert_eq!(
            returns(&format!("https://192-168-1-10.{OWN}:7980/sign-in")),
            named,
            "{case}"
        );
        assert!(returns(&format!("{hostname}/sign-in")), "{case}");
        assert!(!returns(&format!("{public}/sign-in")), "{case}");
    }
}
