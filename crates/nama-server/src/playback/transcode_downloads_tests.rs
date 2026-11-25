use axum::http::{
    HeaderValue, Response, StatusCode,
    header::{CONTENT_LENGTH, CONTENT_RANGE},
};

use super::length_to_the_end;

fn parts(status: StatusCode, length: &'static str, range: Option<&'static str>) -> super::Parts {
    let mut response = Response::new(());
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CONTENT_LENGTH, HeaderValue::from_static(length));
    if let Some(range) = range {
        headers.insert(CONTENT_RANGE, HeaderValue::from_static(range));
    }
    response.into_parts().0
}

#[test]
fn a_whole_file_or_a_range_to_its_end_sends_the_end() {
    assert_eq!(
        length_to_the_end(&parts(StatusCode::OK, "100", None)),
        Some(100)
    );
    let to_the_end = parts(StatusCode::PARTIAL_CONTENT, "90", Some("bytes 10-99/100"));
    assert_eq!(length_to_the_end(&to_the_end), Some(90));
}

#[test]
fn a_range_short_of_the_end_or_a_refusal_does_not_send_the_end() {
    let short = parts(StatusCode::PARTIAL_CONTENT, "99", Some("bytes 0-98/100"));
    assert_eq!(length_to_the_end(&short), None);
    let unsatisfiable = parts(StatusCode::RANGE_NOT_SATISFIABLE, "0", Some("bytes */100"));
    assert_eq!(length_to_the_end(&unsatisfiable), None);
    assert_eq!(
        length_to_the_end(&parts(StatusCode::NOT_MODIFIED, "0", None)),
        None
    );
}
