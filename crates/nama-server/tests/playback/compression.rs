//! How a Remux's files are encoded for what a Device accepts.

use std::io::Read as _;

use flate2::read::GzDecoder;

use super::*;

const ACCEPTS_GZIP: [(&str, &str); 1] = [("accept-encoding", "gzip")];

/// A long Movie's playlist grows a line per segment, and a Device may fetch it again as it plays.
#[tokio::test]
async fn a_remuxs_playlist_is_gzipped_for_a_device_that_accepts_it() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let plan = fx.plan(version).await.unwrap();
    let token = Some(fx.server.admin_token.as_str());

    let gzipped = fetch_with(&fx.server, &plan.url, token, &ACCEPTS_GZIP).await;

    assert_eq!(gzipped.status, 200);
    assert_eq!(gzipped.header("content-encoding"), "gzip");
    assert_eq!(gzipped.header("vary"), "accept-encoding");
    let mut decoded = String::new();
    GzDecoder::new(&gzipped.body[..])
        .read_to_string(&mut decoded)
        .unwrap();
    assert_eq!(decoded.into_bytes(), fx.fetch_ok(&plan.url).await);
}

#[tokio::test]
async fn a_remuxs_playlist_is_sent_as_is_to_a_device_that_accepts_no_encoding() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let plan = fx.plan(version).await.unwrap();
    let token = Some(fx.server.admin_token.as_str());

    let reply = fetch(&fx.server, &plan.url, token).await;

    assert_eq!(reply.status, 200);
    assert!(!reply.headers.contains_key("content-encoding"));
    assert_eq!(reply.header("content-length"), reply.body.len().to_string());
    let playlist = String::from_utf8(reply.body).unwrap();
    assert!(playlist.starts_with("#EXTM3U\n"), "{playlist}");
    assert!(playlist.ends_with("#EXT-X-ENDLIST\n"), "{playlist}");
}

/// MP4 is compressed already, so gzip would cost the Server CPU for nothing, and a request ruling
/// out every encoding, even none, still gets them rather than a 406.
#[tokio::test]
async fn a_remuxs_init_and_segments_are_sent_as_is_whatever_a_device_accepts() {
    let (fx, version) = Fixture::alien_mkv(system_ffmpeg(), "aac").await;
    let plan = fx.plan(version).await.unwrap();
    let token = Some(fx.server.admin_token.as_str());

    for accepted in ["gzip", "identity;q=0"] {
        for file in ["init.mp4", "0.m4s"] {
            let url = beside(&plan.url, file);
            let accepts = [("accept-encoding", accepted)];
            let reply = fetch_with(&fx.server, &url, token, &accepts).await;

            assert_eq!(reply.status, 200, "{file} {accepted}");
            assert_eq!(reply.header("content-type"), "video/mp4", "{file}");
            assert!(!reply.headers.contains_key("content-encoding"), "{file}");
        }
    }
}
