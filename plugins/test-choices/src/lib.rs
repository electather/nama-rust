//! A Built-in Plugin used only in tests (the Server's `testing` feature), standing in for one such as
//! Radarr or Sonarr: its settings check offers choices, its settings can tell it to fail, and it
//! provides a Fulfiller for Movies and one for Shows.

use std::{collections::HashMap, future, sync::Arc};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    fulfiller::{Download, Fulfilled, Fulfiller, Movies, Shows},
    settings::{
        Choice, Choices, SettingField, SettingKind, Settings, SettingsCheck, SettingsSchema,
    },
};
use nama_proto::nama::v1::{Request, request::MovieOrSeason};

/// The test Plugin's id.
pub const ID: &str = "test-choices";

/// The only API key its check accepts.
pub const API_KEY: &str = "test-choices-key-that-must-stay-on-the-server";

/// What `fail_with` is set to for the check to panic.
pub const PANIC: &str = "panic";

/// What `fail_with` is set to for the check never to finish.
pub const HANG: &str = "hang";

/// The test Plugin. It registers its Fulfillers.
#[derive(Debug)]
pub struct TestChoices;

impl Plugin for TestChoices {
    fn manifest(&self) -> Manifest {
        let field = |key: &str, label: &str, description: &str, kind, required| SettingField {
            key: key.to_owned(),
            label: label.to_owned(),
            description: description.to_owned(),
            kind,
            required,
            default: None,
        };
        let settings = SettingsSchema {
            fields: vec![
                field(
                    "api_key",
                    "API key",
                    "The key the check accepts.",
                    SettingKind::Secret,
                    true,
                ),
                field(
                    "fail_with",
                    "Fail with",
                    "Makes the check fail with this error, or panic or hang for those words.",
                    SettingKind::Text,
                    false,
                ),
                field(
                    "profile",
                    "Profile",
                    "One of the profiles the check offers.",
                    SettingKind::Choice,
                    true,
                ),
            ],
        };
        Manifest {
            id: PluginId::from_static(ID),
            name: "Test choices".to_owned(),
            description:
                "Stands in for a Fulfiller in tests, offering choices when its settings are tested."
                    .to_owned(),
            version: "0.0.0".to_owned(),
            settings,
            extension_points: vec![
                PointKey::provide::<dyn Fulfiller<Movies>>(),
                PointKey::provide::<dyn Fulfiller<Shows>>(),
            ],
            logo_svg: Some(include_str!("../logo.svg").to_owned()),
        }
    }

    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        Some(self)
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(Self));
        registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, Arc::new(Self));
    }
}

/// Takes every Request, giving `movie-<TMDB id>` as its reference.
impl Fulfiller<Movies> for TestChoices {
    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        let fulfilled = match &request.movie_or_season {
            Some(MovieOrSeason::Movie(movie)) => Ok(Fulfilled {
                reference: format!("movie-{}", movie.tmdb_id),
                ..Fulfilled::default()
            }),
            _ => Err("not a Movie Request".into()),
        };
        Box::pin(future::ready(fulfilled))
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(future::ready(Ok(HashMap::new())))
    }
}

/// Takes every Request, giving `show-<TMDB id>-season-<number>` as its reference.
impl Fulfiller<Shows> for TestChoices {
    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        let fulfilled = match &request.movie_or_season {
            Some(MovieOrSeason::Season(season)) => Ok(Fulfilled {
                reference: format!(
                    "show-{}-season-{}",
                    season.show_tmdb_id, season.season_number
                ),
                ..Fulfilled::default()
            }),
            _ => Err("not a Season Request".into()),
        };
        Box::pin(future::ready(fulfilled))
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(future::ready(Ok(HashMap::new())))
    }
}

impl SettingsCheck for TestChoices {
    fn check<'a>(&'a self, settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>> {
        #[expect(
            clippy::panic,
            reason = "it panics when told to, to test that the Kernel contains it"
        )]
        Box::pin(async move {
            match settings.text("fail_with") {
                Some(PANIC) => panic!("the test Plugin was told to panic"),
                Some(HANG) => future::pending().await,
                Some(error) => return Err(error.into()),
                None => {}
            }
            if settings.text("api_key") != Some(API_KEY) {
                return Err("the service refused the API key".into());
            }
            let profile = |value: &str, label: &str| Choice {
                value: value.to_owned(),
                label: label.to_owned(),
            };
            Ok(Choices::from([(
                "profile".to_owned(),
                vec![profile("1", "HD"), profile("2", "Ultra HD")],
            )]))
        })
    }
}
