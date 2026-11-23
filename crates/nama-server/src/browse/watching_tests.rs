use std::{sync::Arc, time::Duration};

use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::PageRequest;
use tempfile::TempDir;

use crate::{
    browse::{
        Browse, Started, Target,
        tests::{KNIGHT_TMDB_ID, ada, knight_episode, knight_match, library, store},
    },
    db::{self, Database},
    library::{
        Change, Contents, LibraryKind, Progress,
        fixture::{MatchedMovie, tmdb_movie},
    },
    plugins::ExtensionPoints,
};

/// An Episode by its Season's number and its own.
type Numbered = (i64, i64);

/// A special, then two Seasons, numbered across them as some Shows are.
const EPISODES: [Numbered; 7] = [(0, 1), (1, 1), (1, 2), (1, 3), (1, 4), (2, 5), (2, 6)];

/// A Shows Library of A Knight of the Seven Kingdoms, each Episode in a file of its own, and Ada,
/// a User.
struct Knight {
    db: Arc<Database>,
    library: i64,
    root: String,
    user: i64,
    browse: Browse,
    _dirs: [TempDir; 2],
}

impl Knight {
    async fn with(episodes: &[Numbered]) -> Self {
        let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let (library, root) = library(&db, "Shows", LibraryKind::Shows, &folder).await;
        let knight = Self {
            user: ada(&db).await,
            browse: Browse::new(Arc::clone(&db), ExtensionPoints::default()),
            db,
            library,
            root,
            _dirs: [data, folder],
        };
        for &episode in episodes {
            knight.add(episode).await;
        }
        knight
    }

    /// Stores `episode` as a Scan finding its file would, and returns its Version.
    async fn add(&self, (season, number): Numbered) -> i64 {
        let path = format!(
            "{}/A Knight of the Seven Kingdoms/Season {season:02}/S{season:02}E{number:02}.mkv",
            self.root
        );
        let matched = knight_match(episode_id((season, number)));
        let item = vec![(knight_episode(season, number), matched)];
        store(&self.db, self.library, path, item).await
    }

    async fn hide(&self, target: Target, hidden: bool) {
        (self.browse.hide(self.user, self.library, target, hidden))
            .await
            .unwrap();
    }

    async fn mark_unplayed(&self, episode: Numbered) {
        let episode = Target::Episode(ExternalId::tmdb(episode_id(episode)));
        (self.browse.mark(self.user, self.library, episode, false))
            .await
            .unwrap();
    }

    async fn mark_played(&self, target: Target) {
        (self.browse.mark(self.user, self.library, target, true))
            .await
            .unwrap();
    }

    /// Marks each of `episodes` played, one after another.
    async fn play(&self, episodes: &[Numbered]) {
        for &episode in episodes {
            let id = ExternalId::tmdb(episode_id(episode));
            self.mark_played(Target::Episode(id)).await;
        }
    }

    /// Ada's Continue Watching: each Episode, with how far in she is if she started it.
    async fn continuing(&self) -> Vec<(Numbered, Option<i64>)> {
        (self.page(PageRequest::default()).await.0.into_iter())
            .map(|started| match started {
                Started::Episode { episode, state, .. } => (
                    (episode.season, episode.number),
                    state.map(|state| state.position_ms),
                ),
                Started::Movie { .. } => panic!("only Episodes were watched"),
            })
            .collect()
    }

    /// The page `asked` of Ada's Continue Watching, and the token of the next page.
    async fn page(&self, asked: PageRequest) -> (Vec<Started>, String) {
        let (listed, paging) = (self.browse)
            .continue_watching(self.user, &asked)
            .await
            .unwrap();
        (listed, paging.next_token)
    }
}

/// The TMDB id of the Movie or Episode `started` lists.
fn tmdb_id(started: &Started) -> Option<ExternalId> {
    match started {
        Started::Movie { movie, .. } => movie.ids.get("tmdb"),
        Started::Episode { episode, .. } => episode.ids.get("tmdb"),
    }
}

fn episode_id((season, number): Numbered) -> i64 {
    7_000_000 + season * 100 + number
}

#[tokio::test]
async fn the_episode_after_the_last_one_played_is_continued() {
    let knight = Knight::with(&EPISODES).await;

    knight.play(&[(1, 1), (1, 2), (1, 3)]).await;

    assert_eq!(knight.continuing().await, [((1, 4), None)]);
}

#[tokio::test]
async fn the_next_episode_follows_the_one_played_most_recently() {
    let knight = Knight::with(&EPISODES).await;
    knight.play(&[(1, 1), (1, 2), (1, 3)]).await;

    db::advance_clock(Duration::from_secs(2));
    knight.play(&[(2, 5)]).await;
    assert_eq!(knight.continuing().await, [((2, 6), None)]);

    // Played is when it became played, so playing S01E01 again doesn't move it.
    db::advance_clock(Duration::from_secs(2));
    knight.play(&[(1, 1)]).await;
    assert_eq!(knight.continuing().await, [((2, 6), None)]);
}

#[tokio::test]
async fn a_started_episode_is_its_shows_only_entry_with_its_progress() {
    let all_but_the_fourth = EPISODES.into_iter().filter(|&episode| episode != (1, 4));
    let knight = Knight::with(&all_but_the_fourth.collect::<Vec<_>>()).await;
    let fourth = knight.add((1, 4)).await;
    knight.play(&[(1, 1), (1, 2), (1, 3)]).await;

    let halfway = Progress::new(1_000, 2_000).unwrap();
    knight
        .browse
        .report(knight.user, fourth, halfway)
        .await
        .unwrap();

    assert_eq!(knight.continuing().await, [((1, 4), Some(1_000))]);
}

#[tokio::test]
async fn a_show_played_through_comes_back_once_a_scan_adds_its_next_episode() {
    let knight = Knight::with(&EPISODES).await;

    let show = Target::Show(ExternalId::tmdb(KNIGHT_TMDB_ID));
    knight.mark_played(show).await;
    assert_eq!(knight.continuing().await, []);

    knight.add((2, 7)).await;
    assert_eq!(knight.continuing().await, [((2, 7), None)]);
}

#[tokio::test]
async fn a_special_is_never_the_next_episode() {
    let knight = Knight::with(&EPISODES).await;

    for number in [1, 2] {
        let show = ExternalId::tmdb(KNIGHT_TMDB_ID);
        knight.mark_played(Target::Season { show, number }).await;
    }
    assert_eq!(
        knight.continuing().await,
        [],
        "the unplayed special isn't next"
    );

    db::advance_clock(Duration::from_secs(2));
    knight.play(&[(0, 1)]).await;
    assert_eq!(
        knight.continuing().await,
        [],
        "nor is one after the special"
    );
}

#[tokio::test]
async fn nothing_is_before_a_special_not_even_the_special_before_it() {
    let knight = Knight::with(&[(0, 1), (0, 2), (1, 1)]).await;
    let show = ExternalId::tmdb(KNIGHT_TMDB_ID);

    let previous = (knight.browse)
        .episode_before(knight.user, knight.library, &show, (0, 2))
        .await
        .unwrap();

    assert!(previous.is_none());
}

#[tokio::test]
async fn a_show_pages_among_the_rest_by_its_latest_watch_state_change() {
    let knight = Knight::with(&EPISODES).await;
    let movies = tempfile::tempdir().unwrap();
    let alien = MatchedMovie::new(&knight.db, movies.path(), tmdb_movie(348, "Alien")).await;
    knight.play(&[(1, 1)]).await;

    db::advance_clock(Duration::from_secs(2));
    let halfway = Progress::new(1_000, 2_000).unwrap();
    (knight.browse.report(knight.user, alien.version, halfway))
        .await
        .unwrap();
    db::advance_clock(Duration::from_secs(2));
    knight.play(&[(1, 2)]).await;

    let one = |token: &str| PageRequest {
        size: 1,
        token: token.to_owned(),
        ..Default::default()
    };
    let (first, token) = knight.page(one("")).await;
    let (second, token) = knight.page(one(&token)).await;
    let listed: Vec<_> = first.iter().chain(&second).map(tmdb_id).collect();
    let next = ExternalId::tmdb(episode_id((1, 3)));
    let expected = [Some(next), Some(ExternalId::tmdb(348))];
    assert_eq!(listed, expected, "the Show's S01E02 is newer than Alien");
    assert_eq!(token, "", "the last page");
}

/// The User `user`'s Continue Watching, by each title's TMDB id.
async fn continuing_ids(browse: &Browse, user: i64) -> Vec<Option<ExternalId>> {
    let (listed, _) = (browse.continue_watching(user, &PageRequest::default()))
        .await
        .unwrap();
    listed.iter().map(tmdb_id).collect()
}

/// Ada, halfway through Alien in a Movies Library of its own on `movies`, which she hides.
async fn hide_started_alien(knight: &Knight, movies: &TempDir) -> MatchedMovie {
    let alien = MatchedMovie::new(&knight.db, movies.path(), tmdb_movie(348, "Alien")).await;
    let (browse, user) = (&knight.browse, knight.user);
    browse.report(user, alien.version, halfway()).await.unwrap();
    let movie = Target::Movie(ExternalId::tmdb(348));
    browse.hide(user, alien.library, movie, true).await.unwrap();
    alien
}

fn halfway() -> Progress {
    Progress::new(1_000, 2_000).unwrap()
}

#[tokio::test]
async fn a_hidden_movie_is_left_out_with_its_resume_point_kept_until_it_is_played_again() {
    let (knight, movies) = (Knight::with(&[]).await, tempfile::tempdir().unwrap());
    let (browse, user) = (&knight.browse, knight.user);

    let alien = hide_started_alien(&knight, &movies).await;
    assert_eq!(continuing_ids(browse, user).await, []);
    let (_, state, _) = (browse.movie(user, alien.library, &ExternalId::tmdb(348)))
        .await
        .unwrap();
    let state = state.unwrap();
    assert_eq!((state.position_ms, state.played), (1_000, false));

    db::advance_clock(Duration::from_secs(2));
    browse.report(user, alien.version, halfway()).await.unwrap();
    assert_eq!(
        continuing_ids(browse, user).await,
        [Some(ExternalId::tmdb(348))]
    );
}

#[tokio::test]
async fn a_hidden_show_is_left_out_until_any_of_its_episodes_is_played_again() {
    let knight = Knight::with(&EPISODES).await;
    knight.play(&[(1, 1), (1, 2), (1, 3)]).await;

    knight.hide(knight_show(), true).await;
    assert_eq!(knight.continuing().await, []);

    db::advance_clock(Duration::from_secs(2));
    knight.play(&[(1, 1)]).await;
    assert_eq!(knight.continuing().await, [((1, 4), None)]);
}

#[tokio::test]
async fn a_hidden_show_comes_back_when_one_of_its_episodes_is_marked_unplayed() {
    let knight = Knight::with(&EPISODES).await;
    knight.play(&[(1, 1), (1, 2)]).await;
    knight.hide(knight_show(), true).await;

    knight.mark_unplayed((1, 2)).await;

    assert_eq!(knight.continuing().await, [((1, 2), None)]);
}

#[tokio::test]
async fn marking_an_unstarted_episode_unplayed_leaves_its_show_hidden() {
    let knight = Knight::with(&EPISODES).await;
    knight.play(&[(1, 1), (1, 2)]).await;
    knight.hide(knight_show(), true).await;

    knight.mark_unplayed((1, 4)).await;

    assert_eq!(knight.continuing().await, []);
}

#[tokio::test]
async fn a_new_episode_doesnt_bring_back_a_hidden_show() {
    let knight = Knight::with(&EPISODES).await;
    knight
        .play(&[(1, 1), (1, 2), (1, 3), (1, 4), (2, 5), (2, 6)])
        .await;
    knight.add((2, 7)).await;

    knight.hide(knight_show(), true).await;
    knight.add((2, 8)).await;

    assert_eq!(knight.continuing().await, []);
}

#[tokio::test]
async fn showing_a_hidden_show_again_undoes_hiding_it() {
    let knight = Knight::with(&EPISODES).await;
    knight.play(&[(1, 1)]).await;

    knight.hide(knight_show(), true).await;
    knight.hide(knight_show(), false).await;

    assert_eq!(knight.continuing().await, [((1, 2), None)]);
}

#[tokio::test]
async fn a_hidden_movie_stays_hidden_when_a_new_version_replaces_its_old_one() {
    let (knight, movies) = (Knight::with(&[]).await, tempfile::tempdir().unwrap());
    let alien = hide_started_alien(&knight, &movies).await;

    let contents = Contents::new(Arc::clone(&knight.db));
    let remux = movies
        .path()
        .canonicalize()
        .unwrap()
        .join("Alien remux.mkv");
    let remux = remux.to_str().unwrap();
    MatchedMovie::store(&contents, alien.library, remux, tmdb_movie(348, "Alien")).await;
    let removed = vec![Change::Remove(alien.version)];
    contents.apply(alien.library, removed).await.unwrap();

    assert_eq!(continuing_ids(&knight.browse, knight.user).await, []);
}

fn knight_show() -> Target {
    Target::Show(ExternalId::tmdb(KNIGHT_TMDB_ID))
}

#[tokio::test]
async fn marking_a_started_movie_played_leaves_it_out_and_keeps_a_started_episode() {
    let (knight, movies) = (Knight::with(&[]).await, tempfile::tempdir().unwrap());
    let alien = MatchedMovie::new(&knight.db, movies.path(), tmdb_movie(348, "Alien")).await;
    let (browse, user) = (&knight.browse, knight.user);
    let halfway = Progress::new(1_000, 2_000).unwrap();
    browse.report(user, alien.version, halfway).await.unwrap();
    let episode = knight.add((1, 1)).await;
    browse.report(user, episode, halfway).await.unwrap();

    let movie = Target::Movie(ExternalId::tmdb(348));
    browse.mark(user, alien.library, movie, true).await.unwrap();

    let started = Some(ExternalId::tmdb(episode_id((1, 1))));
    assert_eq!(continuing_ids(browse, user).await, [started]);
}
