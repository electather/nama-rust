use std::sync::Arc;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use nama_proto::nama::v1::Permission;
use tempfile::TempDir;

use super::TitlePages;
use crate::{
    browse::{
        Browse,
        tests::{KNIGHT_TMDB_ID, ada, knight_episode, knight_match, library, store},
    },
    db::Database,
    events::EventBus,
    library::{
        Contents, Decisions, Libraries, LibraryKind, Metadata, OnServer, SimilarAnswer,
        SimilarSources, SourceEpisode, SourceSeason, TitleKind,
        fixture::{MatchedMovie, tmdb_movie},
        scripted::{ScriptedSource, SourcesAsked},
    },
    plugins::Registry,
    requests::Requests,
    server_settings::ServerUrl,
    users::{Permissions, User},
    watchlist,
};

const ALIEN_TMDB_ID: i64 = 348;
const ALIENS_TMDB_ID: i64 = 679;
const PREDATOR_TMDB_ID: i64 = 106;

/// A Server's database, its Plugins, and the pages they show.
struct Fixture {
    _data: TempDir,
    media: TempDir,
    db: Arc<Database>,
    plugins: Registry,
    pages: TitlePages,
}

impl Fixture {
    async fn new() -> Self {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let plugins = Registry::new();
        let points = plugins.extension_points();
        let events = EventBus::new(Arc::clone(&db), points.clone(), ServerUrl::default());
        let requests = Requests::new(Arc::clone(&db), events, points.clone());
        let browse = Browse::new(Arc::clone(&db), points.clone());
        let pages = TitlePages::new(
            Arc::clone(&db),
            browse,
            requests,
            SimilarSources::new(points),
        );
        Self {
            _data: data,
            media: tempfile::tempdir().unwrap(),
            db,
            plugins,
            pages,
        }
    }

    /// Ada, a User of the Server holding `permissions`.
    async fn ada(&self, permissions: &[Permission]) -> User {
        User {
            id: ada(&self.db).await,
            name: "Ada".to_owned(),
            permissions: Permissions::from_iter(permissions.iter().copied()),
            created_at: 0,
        }
    }

    /// A Movies Library holding Alien, Matched to `alien`.
    async fn alien(&self, alien: Metadata) -> MatchedMovie {
        MatchedMovie::new(&self.db, self.media.path(), alien).await
    }

    /// Stores another Movie in `library`, Matched to `matched`.
    async fn store(&self, library: i64, matched: Metadata) {
        let root = self.media.path().canonicalize().unwrap();
        let path = root.join(format!("{}.mkv", matched.title));
        let contents = Contents::new(Arc::clone(&self.db));
        MatchedMovie::store(&contents, library, path.to_str().unwrap(), matched).await;
    }
}

fn imdb(id: &str) -> ExternalId {
    ExternalId::new("imdb", id).unwrap()
}

fn titles(similar: &[OnServer]) -> Vec<&str> {
    (similar.iter())
        .map(|title| match title {
            OnServer::Movie(movie) => movie.title.as_str(),
            OnServer::Show(show) => show.title.as_str(),
        })
        .collect()
}

#[tokio::test]
async fn a_library_page_says_a_title_is_on_the_watchlist_under_any_of_its_ids() {
    let fx = Fixture::new().await;
    let alien = fx
        .alien(Metadata {
            ids: ExternalIds::from_iter([ExternalId::tmdb(ALIEN_TMDB_ID), imdb("tt0078748")]),
            ..tmdb_movie(ALIEN_TMDB_ID, "Alien")
        })
        .await;
    let ada = fx.ada(&[]).await;
    watchlist::add(&fx.db, ada.id, TitleKind::Movie, &imdb("tt0078748"))
        .await
        .unwrap();

    let page = (fx.pages)
        .movie(&ada, alien.library, &ExternalId::tmdb(ALIEN_TMDB_ID))
        .await
        .unwrap();

    assert!(page.on_watchlist);
}

#[tokio::test]
async fn a_found_page_says_a_title_is_on_the_watchlist_only_by_the_id_it_was_looked_up_by() {
    let fx = Fixture::new().await;
    let tmdb = ScriptedSource::new("tmdb", &SourcesAsked::default())
        .looking_up(&["imdb"])
        .with_movie(&["tmdb-348", "imdb-tt0078748"], "Alien");
    fx.plugins.enable(Arc::new(tmdb)).unwrap();
    let ada = fx.ada(&[]).await;
    watchlist::add(&fx.db, ada.id, TitleKind::Movie, &imdb("tt0078748"))
        .await
        .unwrap();

    let by_another_id = (fx.pages)
        .found_movie(&ada, &ExternalId::tmdb(ALIEN_TMDB_ID))
        .await
        .unwrap();
    let by_listed_id = (fx.pages)
        .found_movie(&ada, &imdb("tt0078748"))
        .await
        .unwrap();

    assert!(!by_another_id.on_watchlist, "#1292: not by its other ids");
    assert!(by_listed_id.on_watchlist);
}

#[tokio::test]
async fn a_library_pages_stored_similar_titles_leave_out_a_disabled_sources_answers() {
    let fx = Fixture::new().await;
    let answer = |plugin: &'static str, tmdb: i64| SimilarAnswer {
        plugin: PluginId::from_static(plugin),
        titles: vec![(ExternalId::tmdb(tmdb), 1.0)],
    };
    let alien = fx
        .alien(Metadata {
            similar: vec![
                answer("enabled", ALIENS_TMDB_ID),
                answer("disabled", PREDATOR_TMDB_ID),
            ],
            ..tmdb_movie(ALIEN_TMDB_ID, "Alien")
        })
        .await;
    fx.store(alien.library, tmdb_movie(ALIENS_TMDB_ID, "Aliens"))
        .await;
    fx.store(alien.library, tmdb_movie(PREDATOR_TMDB_ID, "Predator"))
        .await;
    for source in ["enabled", "disabled"] {
        let source = ScriptedSource::new(source, &SourcesAsked::default())
            .with_similar(&format!("tmdb-{ALIENS_TMDB_ID}"), 1.0);
        fx.plugins.enable(Arc::new(source)).unwrap();
    }
    assert!(fx.plugins.disable(&PluginId::from_static("disabled")).await);

    let page = (fx.pages)
        .movie(
            &fx.ada(&[]).await,
            alien.library,
            &ExternalId::tmdb(ALIEN_TMDB_ID),
        )
        .await
        .unwrap();

    assert_eq!(titles(&page.similar), ["Aliens"]);
}

#[tokio::test]
async fn a_found_page_asks_its_similar_sources_live() {
    let fx = Fixture::new().await;
    let aliens = fx.alien(tmdb_movie(ALIENS_TMDB_ID, "Aliens")).await;
    fx.store(aliens.library, tmdb_movie(PREDATOR_TMDB_ID, "Predator"))
        .await;
    let tmdb = ScriptedSource::new("tmdb", &SourcesAsked::default())
        .with_movie(&["tmdb-348"], "Alien")
        .with_similar(&format!("tmdb-{ALIENS_TMDB_ID}"), 1.0);
    fx.plugins.enable(Arc::new(tmdb)).unwrap();

    let page = (fx.pages)
        .found_movie(&fx.ada(&[]).await, &ExternalId::tmdb(ALIEN_TMDB_ID))
        .await
        .unwrap();

    assert_eq!(titles(&page.similar), ["Aliens"]);
}

#[tokio::test]
async fn a_shows_page_gives_its_aired_episode_count() {
    let fx = Fixture::new().await;
    let (library, root) = library(&fx.db, "Shows", LibraryKind::Shows, &fx.media).await;
    let path = format!("{root}/A Knight of the Seven Kingdoms/Season 01/S01E01.mkv");
    let pilot = (knight_episode(1, 1), knight_match(5_000_001));
    store(&fx.db, library, path, vec![pilot]).await;
    let episode = |number: u32, air_date: &str| SourceEpisode {
        ids: ExternalIds::default(),
        season: 1,
        number,
        title: format!("Episode {number}"),
        overview: None,
        air_date: Some(air_date.to_owned()),
        runtime_minutes: None,
        still: None,
    };
    let season = SourceSeason {
        ids: ExternalIds::default(),
        number: 1,
        title: "Season 1".to_owned(),
        overview: None,
        air_date: None,
        poster: None,
        episodes: vec![episode(1, "2026-01-18"), episode(2, "2999-01-01")],
    };
    let knight = ExternalIds::from_iter([ExternalId::tmdb(KNIGHT_TMDB_ID)]);
    let seasons = vec![(PluginId::from_static("tmdb"), knight, season)];
    (Contents::new(Arc::clone(&fx.db)))
        .record(Decisions {
            items: Vec::new(),
            seasons,
        })
        .await
        .unwrap();

    let page = (fx.pages)
        .show(
            &fx.ada(&[]).await,
            library,
            &ExternalId::tmdb(KNIGHT_TMDB_ID),
        )
        .await
        .unwrap();

    assert_eq!(page.aired_episodes, 1, "one aired, one yet to");
}

#[tokio::test]
async fn a_user_without_delete_media_is_not_told_the_read_only_folder() {
    let fx = Fixture::new().await;
    let alien = fx.alien(tmdb_movie(ALIEN_TMDB_ID, "Alien")).await;
    let root = fx.media.path().canonicalize().unwrap();
    let root = root.to_str().unwrap().to_owned();
    (Libraries::new(Arc::clone(&fx.db)))
        .set_read_only(alien.library, vec![root.clone()])
        .await
        .unwrap();
    let alien_id = ExternalId::tmdb(ALIEN_TMDB_ID);

    let without = fx.ada(&[]).await;
    let with = User {
        permissions: Permissions::from_iter([Permission::DeleteMedia]),
        ..without.clone()
    };
    let untold = fx.pages.movie(&without, alien.library, &alien_id).await;
    let told = fx.pages.movie(&with, alien.library, &alien_id).await;

    assert_eq!(untold.unwrap().read_only_folder, None);
    assert_eq!(told.unwrap().read_only_folder, Some(root));
}
