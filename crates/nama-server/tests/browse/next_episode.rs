//! `GetNextEpisode`: the Episode that plays after a given one, as autoplay asks for it.

use nama_proto::nama::v1::GetNextEpisodeRequest;

use super::*;

/// An Episode of Dark by its Season's number and its own.
type Numbered = (u32, u32);

/// Dark's Episodes the fake TMDB knows that the fixture writes, each with its TMDB id.
const DARK_EPISODES: [(Numbered, i64); 5] = [
    ((0, 1), 1_915_604),
    ((1, 1), SECRETS),
    ((1, 2), LIES),
    ((1, 3), 1_388_540),
    ((2, 1), BEGINNINGS_AND_ENDINGS),
];

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Shows Library holding `episodes` of Dark, each in a file of its own,
    /// scanned and Matched.
    async fn dark_episodes(episodes: &[Numbered]) -> Self {
        let mut fx = Self::start(LibraryKind::LIBRARY_KIND_SHOWS).await;
        for (season, number) in episodes {
            let path =
                format!("Dark (2017)/Season {season:02}/Dark - S{season:02}E{number:02}.mkv");
            fx.write(&path, b"episode");
        }
        fx.items = fx.scan_and_match().await;
        fx
    }

    /// Marks each of `episodes` of Dark played for the Admin.
    async fn play(&self, episodes: &[Numbered]) {
        let admin = self.admin();
        for numbered in episodes {
            let (_, tmdb) = (DARK_EPISODES.iter())
                .find(|(known, _)| known == numbered)
                .unwrap();
            let played = admin.set_played(self.library, episode(*tmdb), true);
            played.await.unwrap();
        }
    }

    /// The Episode of Dark the Admin's player plays after `after`, by its numbers.
    async fn next_after(&self, after: Numbered) -> Result<Option<Episode>, ConnectError> {
        let (season_number, episode_number) = after;
        let next = self
            .browse
            .get_next_episode(GetNextEpisodeRequest {
                library_id: self.library,
                show_id: tmdb_id(DARK).into(),
                season_number,
                episode_number,
                ..Default::default()
            })
            .await?
            .into_owned()
            .next
            .into_option();
        Ok(next)
    }

    /// The numbers of the Episode the Admin's player plays after `after`.
    async fn numbers_after(&self, after: Numbered) -> Option<Numbered> {
        let next = self.next_after(after).await.unwrap();
        next.map(|episode| (episode.season_number, episode.episode_number))
    }
}

#[tokio::test]
async fn the_next_episode_is_the_one_after_with_its_versions_and_watch_state() {
    let fx = Fixture::dark_episodes(&[(1, 1), (1, 2), (1, 3)]).await;
    fx.play(&[(1, 1), (1, 2)]).await;
    let third = fx.dark_version(1, "Dark - S01E03.mkv").await;
    fx.admin().report(third, MINUTE, AN_HOUR).await.unwrap();

    let next = fx.next_after((1, 2)).await.unwrap().unwrap();

    assert_eq!((next.season_number, next.episode_number), (1, 3));
    assert_eq!(
        versions(&next.versions),
        [("Dark - S01E03.mkv", "episode".len() as u64)]
    );
    let state = next.watch_state.into_option().unwrap();
    assert_eq!((state.position_ms, state.played), (MINUTE, false));
}

#[tokio::test]
async fn after_a_seasons_last_episode_comes_the_next_seasons_first() {
    let fx = Fixture::dark_episodes(&[(1, 2), (1, 3), (2, 1)]).await;

    assert_eq!(fx.numbers_after((1, 3)).await, Some((2, 1)));
}

#[tokio::test]
async fn a_played_episode_is_skipped() {
    let fx = Fixture::dark_episodes(&[(1, 2), (1, 3), (2, 1)]).await;
    fx.play(&[(1, 3)]).await;

    assert_eq!(fx.numbers_after((1, 2)).await, Some((2, 1)));
}

#[tokio::test]
async fn nothing_is_next_once_every_later_episode_is_played_or_there_is_none() {
    let fx = Fixture::dark_episodes(&[(1, 1), (1, 2), (1, 3), (2, 1)]).await;
    fx.play(&[(1, 3), (2, 1)]).await;

    assert_eq!(
        fx.numbers_after((1, 2)).await,
        None,
        "every later one played"
    );
    assert_eq!(fx.numbers_after((2, 1)).await, None, "none later");
}

#[tokio::test]
async fn a_special_is_never_next_and_nothing_follows_one() {
    let fx = Fixture::dark_episodes(&[(0, 1), (1, 1), (1, 2), (1, 3)]).await;

    assert_eq!(
        fx.numbers_after((1, 3)).await,
        None,
        "the unplayed special isn't next"
    );
    assert_eq!(
        fx.numbers_after((0, 1)).await,
        None,
        "nor is the first Episode after the special"
    );
}

#[tokio::test]
async fn an_episode_the_library_lacks_is_never_next() {
    let fx = Fixture::dark_episodes(&[(1, 1), (1, 3)]).await;

    assert_eq!(fx.numbers_after((1, 1)).await, Some((1, 3)));
}

#[tokio::test]
async fn an_unknown_season_or_episode_is_not_found() {
    let fx = Fixture::dark_episodes(&[(1, 1), (1, 3)]).await;

    let season = assert_refused(
        fx.next_after((3, 1)).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_SEASON_NOT_FOUND,
    );
    let episode = assert_refused(
        fx.next_after((1, 2)).await,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_EPISODE_NOT_FOUND,
    );
    assert_eq!(season.field, "season_number");
    assert_eq!(episode.field, "episode_number");
}
