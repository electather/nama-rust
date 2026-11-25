//! Transcoding a Version into a file for a User with `PERMISSION_DOWNLOAD` to keep on their
//! Device, within a Playback Limit, polled until it's made and deleted once fetched, expired or
//! its Version is gone, within the budget `nama.toml` sets.

use std::process::Command;

use http_body_util::Empty;
use hyper::body::Bytes;
use nama_proto::nama::v1::{SessionDelivery, SignInRequest, SignInServiceClient};
use serde_json::Value;

use super::{
    http::{Reply, send},
    server::PASSWORD,
    *,
};

const ALIEN_MKV: &str = "Alien (1979)/Alien (1979).mkv";

/// Alien as 2 seconds of 1080p H.264, on a Server built by `builder` with the ffmpeg on `PATH`,
/// and Ada, who may download.
async fn alien_and_ada(builder: Builder) -> (Fixture, i64, SignedIn) {
    let builder = builder.ffmpeg(system_ffmpeg());
    let (fx, version) = Fixture::alien_written_on(builder, ffmpeg::write_1080p_h264).await;
    let ada = (fx.server)
        .user("ada", &[Permission::PERMISSION_DOWNLOAD])
        .await;
    (fx, version, ada)
}

/// Asks for a file of the Version `version` within `limit`, as the API names it, as `user`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask(fx: &Fixture, user: &SignedIn, version: i64, limit: &str) -> Reply {
    let request = hyper::Request::post(format!("/media/downloads/{version}/transcodes/{limit}"));
    let request = request.body(Empty::<Bytes>::new()).unwrap();
    send(&fx.server, request, Some(&user.token)).await
}

/// Asks for a file of Alien within `limit` as Ada, and polls it until it's made, returning where
/// it's polled and what the poll said last.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn made(fx: &Fixture, ada: &SignedIn, version: i64, limit: &str) -> (String, Value) {
    let asked = ask(fx, ada, version, limit).await;
    assert_eq!(
        asked.status,
        202,
        "{}",
        String::from_utf8_lossy(&asked.body)
    );
    let poll_url = asked.header("location").to_owned();
    let made = tokio::time::timeout(WAIT, async {
        loop {
            let polled = poll(fx, &ada.token, &poll_url).await.unwrap();
            if polled["state"] != "MAKING" {
                return polled;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    let polled = made.await.unwrap();
    assert_eq!(polled["state"], "MADE", "{polled}");
    (poll_url, polled)
}

/// What polling `poll_url` with the session `token` says, or `None` once it's gone (404).
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn poll(fx: &Fixture, token: &str, poll_url: &str) -> Option<Value> {
    let reply = fetch(&fx.server, poll_url, Some(token)).await;
    if reply.status == 404 {
        return None;
    }
    assert_eq!(reply.status, 200);
    Some(serde_json::from_slice(&reply.body).unwrap())
}

/// Signs `name` in on the Device holding the session `token`, which then acts as them, and
/// returns the session it hands out.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn sign_in_on(server: &TestServer, token: &str, name: &str) -> String {
    SignInServiceClient::new(HttpClient::plaintext(), server.client_config_as(token))
        .sign_in(SignInRequest {
            name: name.to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Phone".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .token
}

/// The files made for Devices to download that are in the segment folder.
fn kept_files(fx: &Fixture) -> usize {
    let folder = fx.server.data_dir().join("transcodes/nama-plan-downloads");
    fs::read_dir(folder).map_or(0, Iterator::count)
}

/// What ffprobe says of the first video in the file at `path`, as `key=value` lines.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn probed_video(path: &Path) -> String {
    let probed = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries"])
        .args(["stream=codec_name,width,height:format=format_name,bit_rate,duration"])
        .args(["-of", "default=noprint_wrappers=1"])
        .arg(path)
        .output()
        .unwrap();
    assert!(probed.status.success(), "{probed:?}");
    // ffprobe ends its lines with `\r\n` on Windows.
    String::from_utf8(probed.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

#[tokio::test]
async fn a_user_with_download_gets_a_transcode_that_plays_within_the_limit_then_it_is_deleted() {
    let (fx, version, ada) = alien_and_ada(TestServer::builder()).await;

    let (poll_url, polled) = made(&fx, &ada, version, "PLAYBACK_LIMIT_720P").await;
    let file_url = polled["url"].as_str().unwrap();
    let start = fetch_with(
        &fx.server,
        file_url,
        Some(&ada.token),
        &[("range", "bytes=0-9")],
    )
    .await;
    let kept_after_a_range = poll(&fx, &ada.token, &poll_url).await.is_some();
    let whole = fetch(&fx.server, file_url, Some(&ada.token)).await;

    assert_eq!(start.status, 206);
    assert!(
        kept_after_a_range,
        "a range short of the end isn't the whole fetch"
    );
    assert_eq!(whole.status, 200);
    assert_eq!(start.body, whole.body[..10]);
    assert_eq!(polled["sizeBytes"], whole.body.len());
    let kept = tempfile::tempdir().unwrap();
    let path = kept.path().join("alien.mp4");
    fs::write(&path, &whole.body).unwrap();
    let probed = probed_video(&path);
    assert!(probed.contains("codec_name=h264"), "{probed}");
    assert!(probed.contains("width=1280\nheight=720"), "{probed}");
    let bit_rate: u64 = (probed.lines())
        .find_map(|line| line.strip_prefix("bit_rate="))
        .unwrap()
        .parse()
        .unwrap();
    assert!(bit_rate <= 4_000_000 + 256_000, "{probed}");
    let decoded = Command::new("ffmpeg")
        .args(["-v", "error", "-xerror", "-i"])
        .arg(&path)
        .args(["-f", "null", "-"])
        .status()
        .unwrap();
    assert!(decoded.success(), "the file doesn't play");
    assert_eq!(poll(&fx, &ada.token, &poll_url).await, None);
    // Deleted in the background once the file is sent.
    let deleted = async {
        while kept_files(&fx) > 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(WAIT, deleted).await.unwrap();
}

#[tokio::test]
async fn past_the_budget_a_new_request_is_refused_with_a_clear_error() {
    let budget = TestServer::builder().config(Config {
        download_budget_mib: 1,
        ..Config::default()
    });
    let (fx, version, ada) = alien_and_ada(budget).await;
    let first = ask(&fx, &ada, version, "PLAYBACK_LIMIT_480P").await;

    let second = ask(&fx, &ada, version, "PLAYBACK_LIMIT_720P").await;

    assert_eq!(first.status, 202);
    assert_eq!(second.status, 507);
    let why = String::from_utf8(second.body).unwrap();
    assert!(why.contains("of the 1 MiB budget"), "{why}");
}

#[tokio::test]
async fn an_unfetched_file_is_deleted_once_it_expires() {
    let (fx, version, ada) = alien_and_ada(TestServer::builder().moves_clock()).await;
    let (poll_url, _) = made(&fx, &ada, version, "PLAYBACK_LIMIT_480P").await;
    assert_eq!(kept_files(&fx), 1);

    fx.server.advance_clock(Duration::from_hours(24));

    assert_eq!(poll(&fx, &ada.token, &poll_url).await, None);
    assert_eq!(kept_files(&fx), 0);
}

#[tokio::test]
async fn a_file_is_deleted_once_its_version_is_gone() {
    let (fx, version, ada) = alien_and_ada(TestServer::builder()).await;
    let (poll_url, _) = made(&fx, &ada, version, "PLAYBACK_LIMIT_480P").await;

    fx.remove_file(ALIEN_MKV);
    fx.scan_and_match().await;

    assert_eq!(kept_files(&fx), 0);
    assert_eq!(poll(&fx, &ada.token, &poll_url).await, None);
}

#[tokio::test]
async fn only_the_user_and_device_that_asked_are_answered_about_its_file() {
    let (fx, version, ada) = alien_and_ada(TestServer::builder()).await;
    let (poll_url, polled) = made(&fx, &ada, version, "PLAYBACK_LIMIT_480P").await;
    let file_url = polled["url"].as_str().unwrap();
    let bishop = (fx.server)
        .user("bishop", &[Permission::PERMISSION_DOWNLOAD])
        .await;

    let on_another_device = poll(&fx, &bishop.token, &poll_url).await;
    let fetched_on_another_device = fetch(&fx.server, file_url, Some(&bishop.token)).await;
    // Ada's Device now acts as Bishop (ADR 0106).
    let on_her_device = sign_in_on(&fx.server, &ada.token, "bishop").await;
    let as_bishop_on_her_device = poll(&fx, &on_her_device, &poll_url).await;
    let fetched_as_bishop = fetch(&fx.server, file_url, Some(&on_her_device)).await;

    assert_eq!(on_another_device, None);
    assert_eq!(fetched_on_another_device.status, 404);
    assert_eq!(as_bishop_on_her_device, None);
    assert_eq!(fetched_as_bishop.status, 404);
}

#[tokio::test]
async fn a_user_without_download_is_refused_a_transcode_to_download() {
    let (fx, version, _) = alien_and_ada(TestServer::builder()).await;
    let bishop = fx.server.user_with_default_permissions("bishop").await;

    let asked = ask(&fx, &bishop, version, "PLAYBACK_LIMIT_480P").await;

    assert_eq!(asked.status, 403);
    assert_eq!(kept_files(&fx), 0);
}

// The fake ffmpeg and ffprobe are shell scripts.
#[cfg(unix)]
#[tokio::test]
async fn a_file_still_being_made_once_it_would_have_expired_is_stopped() {
    let bin = tempfile::tempdir().unwrap();
    let hanging = ffmpeg_writing_nothing(bin.path(), "libx264");
    let builder = TestServer::builder().moves_clock().ffmpeg(hanging);
    let fx = Fixture::start_on(LibraryKind::LIBRARY_KIND_MOVIES, builder).await;
    fx.write(ALIEN_1080P, b"video");
    fx.scan_and_match().await;
    let version = fx.version_ids(ALIEN).await[0];
    let ada = (fx.server)
        .user("ada", &[Permission::PERMISSION_DOWNLOAD])
        .await;
    let asked = ask(&fx, &ada, version, "PLAYBACK_LIMIT_480P").await;
    assert_eq!(asked.status, 202);
    let poll_url = asked.header("location").to_owned();
    let making = poll(&fx, &ada.token, &poll_url).await.unwrap();

    fx.server.advance_clock(Duration::from_hours(24));

    assert_eq!(making["state"], "MAKING");
    assert_eq!(poll(&fx, &ada.token, &poll_url).await, None);
}

#[tokio::test]
async fn original_is_no_limit_to_transcode_a_file_to() {
    let (fx, version, ada) = alien_and_ada(TestServer::builder()).await;

    let asked = ask(&fx, &ada, version, "PLAYBACK_LIMIT_ORIGINAL").await;

    assert_eq!(asked.status, 400);
}
