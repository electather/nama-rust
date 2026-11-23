use axum::http::{Extensions, HeaderMap, HeaderValue, header};
use nama_forwarded::RequestSource;

use super::{cookie, presented_token};

fn request(https: bool) -> Extensions {
    let mut extensions = Extensions::new();
    extensions.insert(RequestSource {
        client_ip: [127, 0, 0, 1].into(),
        https,
    });
    extensions
}

fn cookie_header(cookies: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, HeaderValue::from_static(cookies));
    headers
}

fn presented(cookies: &'static str, https: bool) -> Option<String> {
    let headers = cookie_header(cookies);
    presented_token(&headers, &request(https)).map(|presented| presented.token.to_owned())
}

#[test]
fn over_https_the_session_cookie_is_host_prefixed() {
    let set = cookie("token", 60, &request(true));

    assert!(set.starts_with("__Host-nama_session=token;"), "{set}");
}

#[test]
fn over_plain_http_the_session_cookie_has_no_prefix() {
    let set = cookie("token", 60, &request(false));

    assert!(set.starts_with("nama_session=token;"), "{set}");
}

#[test]
fn over_https_a_plain_named_cookie_another_host_could_have_set_signs_no_one_in() {
    assert_eq!(presented("nama_session=tossed", true), None);
    assert_eq!(
        presented("nama_session=tossed; __Host-nama_session=own", true),
        Some("own".to_owned())
    );
}

#[test]
fn over_plain_http_only_the_plain_name_is_read() {
    assert_eq!(presented("__Host-nama_session=own", false), None);
    assert_eq!(presented("nama_session=own", false), Some("own".to_owned()));
}
