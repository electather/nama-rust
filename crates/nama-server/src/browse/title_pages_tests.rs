use std::sync::Arc;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::Permission;
use tempfile::TempDir;

use super::TitlePages;
use crate::{
    browse::{Browse, tests::ada},
    db::Database,
    events::EventBus,
    library::{
        Metadata, SimilarSources, TitleKind,
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
}

fn imdb(id: &str) -> ExternalId {
    ExternalId::new("imdb", id).unwrap()
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
