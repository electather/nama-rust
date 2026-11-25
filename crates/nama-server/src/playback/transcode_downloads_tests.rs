use axum::http::{
    HeaderValue, Response, StatusCode,
    header::{CONTENT_LENGTH, CONTENT_RANGE},
};

use std::{path::PathBuf, sync::Arc};

use tokio_util::sync::CancellationToken;

use super::{Download, Stage, StartError, TranscodeDownloads, length_to_the_end};
use crate::{db::Database, library::Contents, server_settings::Settings};

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

#[tokio::test]
async fn a_download_is_refused_once_it_and_those_kept_would_go_past_the_budget() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let settings = Settings::new(Arc::clone(&db), data.path().join("transcodes"));
    let downloads =
        TranscodeDownloads::new(Arc::default(), settings.playback(), Contents::new(db), 1000);

    let first = downloads.reserve(1, taking(600));
    let second = downloads.reserve(2, taking(600));
    let third = downloads.reserve(3, taking(400));

    first.unwrap();
    assert!(matches!(
        second,
        Err(StartError::OverBudget {
            kept: 600,
            budget: 1000,
            needed: 600
        })
    ));
    third.unwrap();
}

/// A file being made that may take `bytes`.
fn taking(bytes: u64) -> Download {
    Download {
        version: 1,
        device: 1,
        file: PathBuf::from("1.mp4"),
        bytes,
        duration_ms: 1000,
        made_ms: Arc::default(),
        stage: Stage::Making,
        stop: CancellationToken::new(),
    }
}
