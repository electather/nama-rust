use std::sync::Arc;

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{
        EpisodeAnswer, EpisodeAsked, ImageAnswer, ImageAsked, MovieAnswer, SearchQuery,
        SearchResults, SeasonAnswer, SeasonAsked, ShowAnswer, TitleAsked,
    },
    metadata::{ExternalId, MetadataError, TitleSource},
    settings::{SettingField, SettingKind, SettingValue, Settings, SettingsSchema},
};

use super::{TitleSources, Unanswered};
use crate::plugins::{Failure, Registry};

/// A title source that answers every question with the error `.0` makes, or with the title unset
/// where it makes none.
struct Failing(fn() -> Option<MetadataError>);

impl Failing {
    fn refuse<T: Default>(&self) -> Result<T, MetadataError> {
        self.0().map_or_else(|| Ok(T::default()), Err)
    }
}

impl TitleSource for Failing {
    fn namespace(&self) -> &'static str {
        "tmdb"
    }

    fn search<'a>(
        &'a self,
        _query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn movie<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn show<'a>(
        &'a self,
        _asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn season<'a>(
        &'a self,
        _asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn episode<'a>(
        &'a self,
        _asked: &'a EpisodeAsked,
    ) -> BoxFuture<'a, Result<EpisodeAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }

    fn image<'a>(
        &'a self,
        _asked: &'a ImageAsked,
    ) -> BoxFuture<'a, Result<ImageAnswer, MetadataError>> {
        Box::pin(async { self.refuse() })
    }
}

struct FailingPlugin(fn() -> Option<MetadataError>);

impl Plugin for FailingPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            settings: SettingsSchema {
                fields: vec![SettingField {
                    key: "api_key".to_owned(),
                    label: String::new(),
                    description: String::new(),
                    kind: SettingKind::Secret,
                    required: false,
                    default: None,
                    min: None,
                    max: None,
                }],
            },
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            ..Manifest::new(PluginId::from_static("failing"))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(Failing(self.0)));
        Ok(())
    }
}

/// What asking for a Show, then for an image from the Plugin `failing`, fails with when its
/// Provider fails with `failure`, or when there is no Provider.
async fn asking(failure: Option<fn() -> Option<MetadataError>>) -> (Unanswered, Unanswered) {
    let plugins = Registry::new();
    if let Some(failure) = failure {
        plugins.enable(Arc::new(FailingPlugin(failure))).unwrap();
    }
    let source = TitleSources::new(plugins.extension_points());
    let show = source.show_by(&ExternalId::tmdb(1399)).await.unwrap_err();
    let image = source
        .image(&PluginId::from_static("failing"), "/poster.jpg")
        .await
        .unwrap_err();
    (show, image)
}

#[tokio::test]
async fn without_a_provider_the_source_is_unavailable() {
    assert_eq!(
        asking(None).await,
        (Unanswered::Unavailable, Unanswered::Unavailable)
    );
}

#[tokio::test]
async fn a_title_the_provider_does_not_have_is_not_found() {
    let failure = || None;

    assert_eq!(
        asking(Some(failure)).await,
        (Unanswered::NotFound, Unanswered::NotFound)
    );
}

#[tokio::test]
async fn a_method_the_source_does_not_serve_is_not_found() {
    let failure = || Some(MetadataError::Unsupported);

    assert_eq!(
        asking(Some(failure)).await,
        (Unanswered::NotFound, Unanswered::NotFound)
    );
}

#[tokio::test]
async fn a_provider_that_fails_is_unavailable() {
    let failure = || Some(MetadataError::Failed("TMDB is unreachable".into()));

    assert_eq!(
        asking(Some(failure)).await,
        (Unanswered::Unavailable, Unanswered::Unavailable)
    );
}

#[tokio::test]
async fn a_providers_failure_comes_without_its_plugins_secret_settings() {
    let plugins = Registry::new();
    let failure = || {
        Some(MetadataError::Failed(
            "GET /tv/1399?api_key=s3cr3t failed".into(),
        ))
    };
    let api_key = SettingValue::Text("s3cr3t".to_owned());
    let settings = Settings::new([("api_key".to_owned(), api_key)].into());
    plugins
        .enable_with_settings(Arc::new(FailingPlugin(failure)), settings)
        .unwrap();

    let answer = plugins
        .ask_first(|metadata: Arc<dyn TitleSource>| async move {
            metadata.show(&TitleAsked::default()).await
        })
        .await
        .unwrap();

    let blanked = Failure {
        kind: Unanswered::Unavailable,
        message: "GET /tv/1399?api_key=… failed".to_owned(),
    };
    assert_eq!(answer.result.unwrap_err(), blanked);
}
