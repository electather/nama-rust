//! Watch State a Device made offline, sent back once it reconnects, over a real socket with the
//! TMDB Plugin enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::time::{SystemTime, UNIX_EPOCH};

use browsing::{Fixture, tmdb_id};
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    OfflineWatchStateChange, RefusalReason, ReportOfflineWatchStateRequest, WatchState,
    WatchStateServiceClient, offline_watch_state_change::Title,
};
use refused::assert_refused;
use server::tmdb::ALIEN;

/// Alien at `position_ms` of two hours, changed `secs_ago` seconds ago.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn alien_at(position_ms: u64, secs_ago: u64) -> OfflineWatchStateChange {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    OfflineWatchStateChange {
        title: Some(Title::MovieId(Box::new(tmdb_id(ALIEN)))),
        position_ms,
        duration_ms: 7_200_000,
        changed_at_ms: i64::try_from((now.as_secs() - secs_ago) * 1000).unwrap(),
        ..Default::default()
    }
}

/// Sends `changes` back as the admin's Device.
async fn send_back(
    fx: &Fixture,
    changes: Vec<OfflineWatchStateChange>,
) -> Result<(), ConnectError> {
    WatchStateServiceClient::new(HttpClient::plaintext(), fx.server.client_config())
        .report_offline_watch_state(ReportOfflineWatchStateRequest {
            changes,
            ..Default::default()
        })
        .await?;
    Ok(())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn alien_state(fx: &Fixture) -> WatchState {
    let movie = fx.get_movie(fx.library, ALIEN).await.unwrap();
    movie.watch_state.into_option().unwrap()
}

#[tokio::test]
async fn an_offline_change_is_stored_as_of_when_it_was_made_and_an_older_one_is_left() {
    let fx = Fixture::alien().await;
    let made = alien_at(60_000, 120);

    send_back(&fx, vec![made.clone()]).await.unwrap();
    send_back(&fx, vec![alien_at(30_000, 180)]).await.unwrap();

    let state = alien_state(&fx).await;
    assert_eq!(
        (state.position_ms, state.updated_at * 1000),
        (60_000, made.changed_at_ms)
    );
}

#[tokio::test]
async fn an_offline_change_with_no_duration_is_refused_naming_it() {
    let fx = Fixture::alien().await;
    let no_duration = OfflineWatchStateChange {
        duration_ms: 0,
        ..alien_at(0, 60)
    };

    let refusal = assert_refused(
        send_back(&fx, vec![alien_at(60_000, 60), no_duration]).await,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_DURATION_OUT_OF_RANGE,
    );

    assert_eq!(refusal.field, "changes[1].duration_ms");
}
