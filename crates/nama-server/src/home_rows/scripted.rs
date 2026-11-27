//! A Home row source a unit test scripts, so it states only the rows it needs, and reads back
//! what the source was asked.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        FeaturedSuggestion, FeaturedSuggestions, HomeRow, HomeRowTitle, HomeRows as Given,
        SharedRowsAsked, TitleId, TitleKind, UserFeaturedAsked, UserRowsAsked,
    },
    home_row::{HomeRowMethod, HomeRowSource},
};

use super::HomeRows;
use crate::{
    db::Database, events::EventBus, plugins::Registry, requests::Requests,
    server_settings::ServerUrl,
};

pub(super) fn movie(value: &str) -> HomeRowTitle {
    titled(TitleKind::TITLE_KIND_MOVIE, "tmdb", value)
}

pub(super) fn titled(kind: TitleKind, namespace: &str, value: &str) -> HomeRowTitle {
    HomeRowTitle {
        kind: kind.into(),
        id: TitleId {
            namespace: namespace.to_owned(),
            value: value.to_owned(),
            ..TitleId::default()
        }
        .into(),
        ..HomeRowTitle::default()
    }
}

pub(super) fn row(key: &str, ttl_seconds: u32, titles: Vec<HomeRowTitle>) -> HomeRow {
    HomeRow {
        key: key.to_owned(),
        heading: key.to_owned(),
        ttl_seconds,
        titles,
        ..HomeRow::default()
    }
}

pub(super) fn given(rows: Vec<HomeRow>) -> Given {
    Given {
        rows,
        ..Given::default()
    }
}

/// What a [`ScriptedRows`] source was asked, and a hold on its answers to Users.
#[derive(Default)]
pub(super) struct Calls {
    /// In order: `None` for the shared rows, or the Personal settings a User's call carried.
    pub(super) asked: Mutex<Vec<Option<BTreeMap<String, String>>>>,
    /// In order, the Personal settings each `user_featured` call carried.
    pub(super) featured_asked: Mutex<Vec<BTreeMap<String, String>>>,
    /// In order, the User id each `user_rows` and `user_featured` call carried.
    pub(super) user_ids: Mutex<Vec<i64>>,
    /// It answers only once it holds this, so a test keeps a call in flight.
    pub(super) held: tokio::sync::Mutex<()>,
}

impl Calls {
    pub(super) fn asked(&self) -> Vec<Option<BTreeMap<String, String>>> {
        self.asked.lock().unwrap().clone()
    }
}

/// A Built-in Home row source, the Plugin `rows`, that records what it is asked and gives the
/// answers its `gives` names: the rows it is told to share and to give each User, and to each User
/// who set a `handle` the suggestion of the Movie of that TMDB id at 0.5.
pub(crate) struct ScriptedRows {
    calls: Arc<Calls>,
    gives: &'static [HomeRowMethod],
    shared: Vec<HomeRow>,
    own: Vec<HomeRow>,
}

impl ScriptedRows {
    /// A source giving the answers `gives` names, with no rows until it is told some.
    pub(crate) fn new(gives: &'static [HomeRowMethod]) -> Self {
        Self {
            calls: Arc::default(),
            gives,
            shared: Vec::new(),
            own: Vec::new(),
        }
    }

    /// It also shares the row `key`, headed `key`, of the Movies of the TMDB ids `movies`, linking
    /// the Library `library` if one.
    pub(crate) fn sharing(mut self, key: &str, library: Option<i64>, movies: &[i64]) -> Self {
        self.shared.push(HomeRow {
            library_id: library,
            ..row(key, 3600, movies_of(movies))
        });
        self
    }

    /// It also gives each User the row `key` of the Movies of the TMDB ids `movies`.
    pub(crate) fn giving_each(mut self, key: &str, movies: &[i64]) -> Self {
        self.own.push(row(key, 3600, movies_of(movies)));
        self
    }

    pub(super) fn calls(&self) -> Arc<Calls> {
        Arc::clone(&self.calls)
    }

    /// Home rows over `db`, with this source enabled, as a Server starts with it.
    pub(crate) fn home_rows(self, db: &Arc<Database>) -> HomeRows {
        let plugins = Registry::new();
        plugins.enable(Arc::new(self)).unwrap();
        let points = plugins.extension_points();
        let events = EventBus::new(Arc::clone(db), points.clone(), ServerUrl::default());
        let requests = Requests::new(Arc::clone(db), events, points.clone());
        HomeRows::new(Arc::clone(db), points, requests)
    }
}

fn movies_of(tmdb: &[i64]) -> Vec<HomeRowTitle> {
    tmdb.iter().map(|id| movie(&id.to_string())).collect()
}

impl Plugin for ScriptedRows {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "1.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn HomeRowSource>()],
            ..Manifest::new(PluginId::from_static("rows"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        let source: Arc<dyn HomeRowSource> = Arc::new(Self {
            calls: Arc::clone(&self.calls),
            gives: self.gives,
            shared: self.shared.clone(),
            own: self.own.clone(),
        });
        registrar.provide(Priority::DEFAULT, source);
        Ok(())
    }
}

impl HomeRowSource for ScriptedRows {
    fn shared_rows<'a>(
        &'a self,
        _asked: &'a SharedRowsAsked,
    ) -> BoxFuture<'a, Result<Given, PluginError>> {
        self.calls.asked.lock().unwrap().push(None);
        Box::pin(async {
            let _held = self.calls.held.lock().await;
            Ok(given(self.shared.clone()))
        })
    }

    fn user_rows<'a>(
        &'a self,
        asked: &'a UserRowsAsked,
    ) -> BoxFuture<'a, Result<Given, PluginError>> {
        let settings = asked.personal_settings.clone().into_iter().collect();
        self.calls.asked.lock().unwrap().push(Some(settings));
        self.calls.user_ids.lock().unwrap().push(asked.user_id);
        Box::pin(async {
            let _held = self.calls.held.lock().await;
            Ok(given(self.own.clone()))
        })
    }

    fn user_featured<'a>(
        &'a self,
        asked: &'a UserFeaturedAsked,
    ) -> BoxFuture<'a, Result<FeaturedSuggestions, PluginError>> {
        let settings = asked.personal_settings.clone().into_iter().collect();
        self.calls.featured_asked.lock().unwrap().push(settings);
        self.calls.user_ids.lock().unwrap().push(asked.user_id);
        let handle = asked.personal_settings.get("handle").cloned();
        Box::pin(async move {
            let _held = self.calls.held.lock().await;
            let suggestion = |tmdb: String| FeaturedSuggestion {
                kind: TitleKind::TITLE_KIND_MOVIE.into(),
                id: movie(&tmdb).id,
                weight: 0.5,
                ..FeaturedSuggestion::default()
            };
            Ok(FeaturedSuggestions {
                suggestions: handle.into_iter().map(suggestion).collect(),
                ttl_seconds: 3600,
                ..FeaturedSuggestions::default()
            })
        })
    }

    fn gives(&self, method: HomeRowMethod) -> bool {
        self.gives.contains(&method)
    }
}

impl HomeRows {
    /// Waits until no refresh it started is running.
    pub(crate) async fn settled(&self) {
        self.refreshes.settled().await;
    }
}
