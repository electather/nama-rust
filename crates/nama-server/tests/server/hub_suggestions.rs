//! The Kernel's `list_hub_*` functions, which Hub Suggestions is built on (ADR 0105): what the Hub,
//! here an in-process fake, suggests to the Linked Server, in the Plugin contract's terms, and
//! nothing on a Server that isn't Linked (ADR 0002).

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_hub.rs"]
mod fake_hub;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::sync::{Arc, Mutex, PoisonError};

use connectrpc::client::HttpClient;
use fake_hub::FakeHub;
use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, Registrar,
    automation::{
        Kernel, Permission,
        functions::{ListHubMostWatchlisted, ListHubRecommended, ListHubSimilar, ListHubTrending},
    },
    contract::{
        HubChartAsked, HubRecommendedAsked, HubTitles, SimilarAsked, SimilarTitles, TitleId,
        TitleKind,
    },
};
use nama_proto::nama::v1::{GetServerLinkRequest, ServerLink, ServerServiceClient};
use nama_server::{config::Config, plugins::Registry};
use server::TestServer;

/// The Kernel's functions the last enable handed an [`Asker`].
type Caught = Arc<Mutex<Option<Arc<dyn Kernel>>>>;

/// A Built-in Plugin that keeps the Kernel's functions it is handed, so a test can call them, and
/// requests the Permissions Hub Suggestions does.
struct Asker(Caught);

impl Plugin for Asker {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            permissions: vec![
                Permission::PERMISSION_READ_LIBRARIES,
                Permission::PERMISSION_READ_WATCH_STATES,
            ],
            ..Manifest::new(PluginId::from_static("asker"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = registrar.kernel();
        Ok(())
    }
}

/// A Server with `config` and an [`Asker`] enabled, and the Kernel's functions it was handed.
#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
async fn asking(config: Config) -> (TestServer, Arc<dyn Kernel>) {
    let caught = Caught::default();
    let plugins = Registry::new();
    let kept = Arc::clone(&caught);
    plugins.describe_builtin(move || Arc::new(Asker(Arc::clone(&kept))));
    let server = TestServer::builder()
        .config(config)
        .plugins(plugins)
        .start()
        .await;
    let kernel = caught
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    (server, kernel.expect("the Asker was enabled"))
}

fn tmdb(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

/// What a Similar source is asked about the Movie `tmdb-603`.
fn the_matrix() -> SimilarAsked {
    SimilarAsked {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: vec![tmdb("603")],
        ..Default::default()
    }
}

fn recommended_to_7() -> HubRecommendedAsked {
    HubRecommendedAsked {
        user_id: 7,
        namespaces: vec!["tmdb".to_owned(), "imdb".to_owned()],
        ..Default::default()
    }
}

/// Each of `titles` as its kind, External ID and weight, such as `show tmdb-1396 0.4`.
fn listed(titles: &HubTitles) -> Vec<String> {
    (titles.titles.iter())
        .map(|title| {
            let kind = match title.kind.as_known() {
                Some(TitleKind::TITLE_KIND_MOVIE) => "movie",
                Some(TitleKind::TITLE_KIND_SHOW) => "show",
                _ => "unknown",
            };
            let id = &title.id;
            format!("{kind} {}-{} {}", id.namespace, id.value, title.weight)
        })
        .collect()
}

/// What each `list_hub_*` function answers `kernel`, which must answer every one.
struct Answers {
    similar: SimilarTitles,
    recommended: HubTitles,
    trending: HubTitles,
    watchlisted: HubTitles,
}

impl Answers {
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    async fn of(kernel: &Arc<dyn Kernel>) -> Self {
        let chart = HubChartAsked::default;
        Self {
            similar: kernel.ask::<ListHubSimilar>(the_matrix()).await.unwrap(),
            recommended: (kernel.ask::<ListHubRecommended>(recommended_to_7()))
                .await
                .unwrap(),
            trending: kernel.ask::<ListHubTrending>(chart()).await.unwrap(),
            watchlisted: kernel.ask::<ListHubMostWatchlisted>(chart()).await.unwrap(),
        }
    }

    /// How many titles each answer holds.
    fn counts(&self) -> [usize; 4] {
        [
            self.similar.titles.len(),
            self.recommended.titles.len(),
            self.trending.titles.len(),
            self.watchlisted.titles.len(),
        ]
    }
}

#[tokio::test]
async fn a_linked_servers_kernel_answers_what_the_hub_suggests() {
    let hub = FakeHub::start().await;
    let (server, kernel) = asking(hub.config()).await;
    hub.link(&server).await;

    let answers = Answers::of(&kernel).await;

    let similar: Vec<_> = (answers.similar.titles.iter())
        .map(|title| {
            (
                title.id.namespace.as_str(),
                title.id.value.as_str(),
                title.weight,
            )
        })
        .collect();
    assert_eq!(similar, [("tmdb", "604", 0.9)]);
    // Not the title of no kind, which nothing could match.
    assert_eq!(
        listed(&answers.recommended),
        ["movie tmdb-604 0.8", "show tmdb-1396 0.4"]
    );
    assert_eq!(listed(&answers.trending), ["movie tmdb-603 0.7"]);
    assert_eq!(
        listed(&answers.watchlisted),
        ["show tmdb-1396 1", "movie tmdb-603 0.5"]
    );
}

#[tokio::test]
async fn a_server_that_isnt_linked_gets_no_suggestions() {
    let hub = FakeHub::start().await;
    let (_server, kernel) = asking(hub.config()).await;

    assert_eq!(Answers::of(&kernel).await.counts(), [0; 4]);
}

#[tokio::test]
async fn a_server_whose_config_names_no_hub_gets_no_suggestions() {
    let (_server, kernel) = asking(Config::default()).await;

    assert_eq!(Answers::of(&kernel).await.counts(), [0; 4]);
}

#[tokio::test]
async fn a_server_the_hub_forgot_gets_no_suggestions() {
    let hub = FakeHub::start().await;
    let (server, kernel) = asking(hub.config()).await;
    hub.link(&server).await;
    hub.forget_server();

    assert_eq!(Answers::of(&kernel).await.counts(), [0; 4]);
    let settings = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let link = (settings.get_server_link(GetServerLinkRequest::default()))
        .await
        .unwrap()
        .into_owned()
        .link;
    // Forgotten, so the Admin can Link it again at once.
    assert_eq!(link.into_option(), Some(ServerLink::default()));
}

#[tokio::test]
async fn every_suggestion_is_refused_while_the_hub_is_down() {
    let hub = FakeHub::start().await;
    let (server, kernel) = asking(hub.config()).await;
    hub.link(&server).await;
    hub.set_down(true);

    let similar = kernel.ask::<ListHubSimilar>(the_matrix()).await;
    let recommended = kernel.ask::<ListHubRecommended>(recommended_to_7()).await;
    let trending = kernel
        .ask::<ListHubTrending>(HubChartAsked::default())
        .await;
    let watchlisted = (kernel.ask::<ListHubMostWatchlisted>(HubChartAsked::default())).await;

    for refused in [
        similar.err(),
        recommended.err(),
        trending.err(),
        watchlisted.err(),
    ] {
        assert_eq!(
            refused.map(|refused| refused.0),
            Some("the Kernel couldn't reach the Hub".to_owned())
        );
    }
}
