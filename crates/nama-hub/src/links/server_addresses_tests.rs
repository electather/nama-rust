use super::*;

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
