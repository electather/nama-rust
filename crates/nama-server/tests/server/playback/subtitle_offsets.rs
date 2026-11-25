//! Subtitle Offsets: kept per User for one subtitle Track of a Version, which later plans give,
//! whatever Tracks are added beside it.

use nama_proto::nama::v1::SetSubtitleOffsetRequest;

use super::*;

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Sets the Admin's offset of the Track at `track` in the Version `version` to `offset_ms`.
    async fn set_offset(
        &self,
        version: i64,
        track: u32,
        offset_ms: i32,
    ) -> Result<(), ConnectError> {
        PlaybackServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .set_subtitle_offset(SetSubtitleOffsetRequest {
                version_id: version,
                track,
                offset_ms,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    /// The place and offset of each subtitle of the Admin's plan for the Version `version`,
    /// stopped once read, as each runs a Transcode.
    async fn offsets(&self, version: i64) -> Vec<(u32, i32)> {
        let plan = self.plan(version).await.unwrap();
        self.stop(plan.id).await;
        places_and_offsets(&plan)
    }
}

/// The place and offset of each of `plan`'s subtitles.
fn places_and_offsets(plan: &PlaybackPlan) -> Vec<(u32, i32)> {
    (plan.subtitles.iter())
        .map(|subtitle| (subtitle.place, subtitle.offset_ms))
        .collect()
}

#[tokio::test]
async fn an_offset_comes_back_in_later_plans_for_its_track_alone_and_its_user_alone() {
    let (fx, version) =
        Fixture::alien_with_sidecars(&[("Alien (1979) - 1080p.en.srt", HELLO_SRT.1)]).await;
    let lin = fx.server.user("Lin", &[]).await;
    let before = fx.offsets(version).await;

    fx.set_offset(version, 2, 1500).await.unwrap();
    fx.set_offset(version, 1, -250).await.unwrap();
    fx.set_offset(version, 2, 1700).await.unwrap();

    assert_eq!(before, [(1, 0), (2, 0)]);
    assert_eq!(fx.offsets(version).await, [(1, -250), (2, 1700)]);
    assert_eq!(
        places_and_offsets(&plan_as(&lin, version).await.unwrap()),
        [(1, 0), (2, 0)]
    );
}

#[tokio::test]
async fn a_sidecar_added_before_one_with_an_offset_starts_at_0_and_leaves_that_offset_on_its_track()
{
    let (fx, version) =
        Fixture::alien_with_sidecars(&[("Alien (1979) - 1080p.en.srt", HELLO_SRT.1)]).await;
    fx.set_offset(version, 2, 1500).await.unwrap();

    // Its path comes first, so it takes place 2.
    fx.rewrite_sidecar("Alien (1979) - 1080p.de.srt", HELLO_SRT.1)
        .await;

    let plan = fx.plan(version).await.unwrap();
    fx.stop(plan.id).await;
    let languages: Vec<_> = (plan.subtitles.iter())
        .map(|subtitle| subtitle.track.language.as_deref())
        .collect();
    assert_eq!(languages, [None, Some("de"), Some("en")]);
    assert_eq!(places_and_offsets(&plan), [(1, 0), (2, 0), (3, 1500)]);
}

#[tokio::test]
async fn an_offset_for_a_place_holding_no_subtitle_is_refused() {
    let (fx, version) = Fixture::alien_with_sidecars::<&str>(&[]).await;

    for place in [0, 2] {
        assert_refused(
            fx.set_offset(version, place, 500).await,
            ErrorCode::NotFound,
            RefusalReason::REFUSAL_REASON_TRACK_NOT_FOUND,
        );
    }
}
