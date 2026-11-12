//! A fake Fulfiller a test turns on through the builder, and the Qualities an Admin offers of it
//! as the test writes them.

use std::{
    collections::HashMap,
    future,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    fulfiller::{
        Download, Fulfilled, Fulfiller, Movies, ShowDeleted, Shows, StopMonitoring, stop_monitoring,
    },
    settings::{Choice, Choices, Settings, SettingsCheck},
};
use nama_proto::nama::v1::{
    Quality, Request, RequestedMovie, RequestedSeason, request::MovieOrSeason,
};
use nama_server::plugins::Registry;

/// The id of the Plugin providing the fake Fulfiller for Movies.
pub(crate) const MOVIE_FULFILLER: &str = "fake-fulfiller";
/// The id of the Plugin providing the fake Fulfiller for Shows.
pub(crate) const SHOWS_FULFILLER: &str = "fake-shows-fulfiller";

/// Which Fulfiller a [`FakeFulfiller`]'s Plugin provides.
#[derive(Clone, Copy)]
pub(crate) enum Fulfills {
    /// The Movie Fulfiller, [`MOVIE_FULFILLER`].
    Movies,
    /// The Fulfiller for Shows, [`SHOWS_FULFILLER`].
    Shows,
}

impl Fulfills {
    /// The Plugin providing `fulfiller` as this Fulfiller.
    pub(crate) fn plugin(self, fulfiller: FakeFulfiller) -> Arc<dyn Plugin> {
        match self {
            Self::Movies => Arc::new(FulfillerPlugin(fulfiller)),
            Self::Shows => Arc::new(ShowsFulfillerPlugin(fulfiller)),
        }
    }
}

/// Makes `fulfiller`'s Plugin, providing what `fulfills` names, a Built-in one of `plugins` and
/// enables it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn enable(plugins: &Registry, fulfills: Fulfills, fulfiller: &FakeFulfiller) {
    let made = fulfiller.clone();
    plugins.describe_builtin(move || fulfills.plugin(made.clone()));
    plugins.enable(fulfills.plugin(fulfiller.clone())).unwrap();
}

/// A Quality an Admin offers, of the service's profile `profile_id`, named `name`.
pub(crate) fn quality(profile_id: &str, name: &str, is_default: bool) -> Quality {
    Quality {
        profile_id: profile_id.to_owned(),
        name: name.to_owned(),
        is_default,
        ..Default::default()
    }
}

/// A Season handed to a [`FakeFulfiller`]: its Show's TMDB and TVDB ids, its number, and whether
/// the hand-off asked to follow the Show.
pub(crate) type HandedSeason = (i64, Option<i64>, u32, bool);

/// A Fulfiller that takes every Request it is handed, giving `movie-<TMDB id>` or
/// `show-<TMDB id>-season-<number>` as its reference, unless it is refusing them all, and says it
/// is downloading what the test set.
#[derive(Clone, Default)]
pub(crate) struct FakeFulfiller {
    handed: Arc<Mutex<Vec<i64>>>,
    handed_seasons: Arc<Mutex<Vec<HandedSeason>>>,
    refuses: Arc<AtomicBool>,
    /// Whether it never answers a hand-off.
    stalls: Arc<AtomicBool>,
    /// Whether it refuses the hand-offs that ask it to follow a Show.
    refuses_following: Arc<AtomicBool>,
    unfollowed: Arc<Mutex<Vec<i64>>>,
    /// The TMDB id of each Movie it was told to stop monitoring, in order.
    stopped_monitoring: Arc<Mutex<Vec<i64>>>,
    /// What of a Show it was told to stop monitoring each time, in order.
    stopped_monitoring_shows: Arc<Mutex<Vec<ShowDeleted>>>,
    /// Whether it panics when told to stop monitoring.
    panics_stopping_monitoring: Arc<AtomicBool>,
    /// What it says it is downloading, by reference.
    downloading: Arc<Mutex<HashMap<String, Vec<Download>>>>,
    /// Whether it fails to say what it is downloading.
    progress_fails: Arc<AtomicBool>,
    /// The service profiles its settings check offers.
    profiles: Arc<Mutex<Vec<String>>>,
    /// Whether its settings check fails.
    check_fails: Arc<AtomicBool>,
    /// The profile id and name it reports it used on each hand-off, if any.
    used: Arc<Mutex<Option<(String, String)>>>,
}

impl FakeFulfiller {
    /// Refuses every Request it is handed, and fails to stop monitoring, from now on.
    pub(crate) fn refuse(&self) {
        self.refuses.store(true, Ordering::Relaxed);
    }

    /// Never answers a hand-off from now on.
    pub(crate) fn stall(&self) {
        self.stalls.store(true, Ordering::Relaxed);
    }

    /// Refuses the hand-offs that ask it to follow a Show from now on.
    pub(crate) fn refuse_to_follow(&self) {
        self.refuses_following.store(true, Ordering::Relaxed);
    }

    /// Takes every Request it is handed from now on.
    pub(crate) fn recover(&self) {
        self.refuses.store(false, Ordering::Relaxed);
        self.refuses_following.store(false, Ordering::Relaxed);
    }

    /// Says it is downloading `downloads` for the Movie `tmdb_id` from now on.
    pub(crate) fn download(&self, tmdb_id: i64, downloads: Vec<Download>) {
        self.downloading
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(format!("movie-{tmdb_id}"), downloads);
    }

    /// Fails to say what it is downloading from now on.
    pub(crate) fn fail_progress(&self) {
        self.progress_fails.store(true, Ordering::Relaxed);
    }

    /// Has its settings check offer the service profiles `profiles` from now on.
    pub(crate) fn offer_profiles(&self, profiles: &[&str]) {
        *self.profiles.lock().unwrap_or_else(PoisonError::into_inner) =
            profiles.iter().map(|&profile| profile.to_owned()).collect();
    }

    /// Has its settings check fail from now on.
    pub(crate) fn fail_checks(&self) {
        self.check_fails.store(true, Ordering::Relaxed);
    }

    /// Reports the profile `profile_id`, named `profile_name`, as the one it used on each hand-off
    /// from now on, as a service that already had the title would.
    pub(crate) fn report_used(&self, profile_id: &str, profile_name: &str) {
        *self.used.lock().unwrap_or_else(PoisonError::into_inner) =
            Some((profile_id.to_owned(), profile_name.to_owned()));
    }

    /// What it fulfilled a hand-off with `reference`.
    fn fulfilled(&self, reference: String) -> Fulfilled {
        let (profile_id, profile_name) = self
            .used
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .unwrap_or_default();
        Fulfilled {
            reference,
            profile_id,
            profile_name,
            ..Fulfilled::default()
        }
    }

    /// The TMDB id of each Movie it was handed, in order.
    pub(crate) fn handed(&self) -> Vec<i64> {
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Each Season it was handed, in order.
    pub(crate) fn handed_seasons(&self) -> Vec<HandedSeason> {
        self.handed_seasons
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Panics when told to stop monitoring from now on, which disables its Plugin.
    pub(crate) fn panic_stopping_monitoring(&self) {
        self.panics_stopping_monitoring
            .store(true, Ordering::Relaxed);
    }

    /// The TMDB id of each Movie it was told to stop monitoring, in order.
    pub(crate) fn stopped_monitoring(&self) -> Vec<i64> {
        self.stopped_monitoring
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// What of a Show it was told to stop monitoring each time, in order.
    pub(crate) fn stopped_monitoring_shows(&self) -> Vec<ShowDeleted> {
        self.stopped_monitoring_shows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The TMDB id of each Show it was told to stop following, in order.
    pub(crate) fn unfollowed(&self) -> Vec<i64> {
        self.unfollowed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The Movie `request` asks for, as the Server hands only those to a Movies Fulfiller.
#[expect(clippy::panic, reason = "a test helper fails its test by panicking")]
fn asked_movie(request: &Request) -> &RequestedMovie {
    match &request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => movie,
        other => panic!("a Movies Fulfiller was handed {other:?}"),
    }
}

/// The Season `request` asks for, as the Server hands only those to a Shows Fulfiller.
#[expect(clippy::panic, reason = "a test helper fails its test by panicking")]
fn asked_season(request: &Request) -> &RequestedSeason {
    match &request.movie_or_season {
        Some(MovieOrSeason::Season(season)) => season,
        other => panic!("a Shows Fulfiller was handed {other:?}"),
    }
}

impl Fulfiller<Movies> for FakeFulfiller {
    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        let tmdb_id = asked_movie(request).tmdb_id;
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tmdb_id);
        let refuses = self.refuses.load(Ordering::Relaxed);
        let stalls = self.stalls.load(Ordering::Relaxed);
        let fulfilled = self.fulfilled(format!("movie-{tmdb_id}"));
        Box::pin(async move {
            if stalls {
                future::pending::<()>().await;
            }
            if refuses {
                return Err("Radarr is unreachable".into());
            }
            Ok(fulfilled)
        })
    }

    #[expect(clippy::panic, reason = "a test helper fails its test by panicking")]
    fn stop_monitoring<'a>(
        &'a self,
        title: &'a StopMonitoring,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let Some(stop_monitoring::Title::MovieTmdbId(tmdb_id)) = title.title else {
            panic!("a Movies Fulfiller was told to stop monitoring {title:?}");
        };
        assert!(
            !self.panics_stopping_monitoring.load(Ordering::Relaxed),
            "the test made it panic"
        );
        self.stopped_monitoring
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tmdb_id);
        let refuses = self.refuses.load(Ordering::Relaxed);
        Box::pin(async move {
            if refuses {
                return Err("Radarr is unreachable".into());
            }
            Ok(())
        })
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        let fails = self.progress_fails.load(Ordering::Relaxed);
        let downloading = self
            .downloading
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Box::pin(async move {
            if fails {
                return Err("Radarr is unreachable".into());
            }
            Ok(downloading)
        })
    }
}

impl Fulfiller<Shows> for FakeFulfiller {
    fn fulfill<'a>(
        &'a self,
        request: &'a Request,
    ) -> BoxFuture<'a, Result<Fulfilled, PluginError>> {
        let season = asked_season(request);
        let (show, season_number) = (season.show_tmdb_id, season.season_number);
        let follow = request.followed_show_id.is_some();
        self.handed_seasons
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((show, season.show_tvdb_id, season_number, follow));
        let refuses = self.refuses.load(Ordering::Relaxed);
        let refuses_to_follow = follow && self.refuses_following.load(Ordering::Relaxed);
        let fulfilled = self.fulfilled(format!("show-{show}-season-{season_number}"));
        Box::pin(async move {
            if refuses {
                return Err("Sonarr is unreachable".into());
            }
            if refuses_to_follow {
                return Err("Sonarr can't follow this Show".into());
            }
            Ok(fulfilled)
        })
    }

    fn unfollow<'a>(&'a self, request: &'a Request) -> BoxFuture<'a, Result<(), PluginError>> {
        self.unfollowed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(asked_season(request).show_tmdb_id);
        Box::pin(async { Ok(()) })
    }

    #[expect(clippy::panic, reason = "a test helper fails its test by panicking")]
    fn stop_monitoring<'a>(
        &'a self,
        title: &'a StopMonitoring,
    ) -> BoxFuture<'a, Result<(), PluginError>> {
        let Some(stop_monitoring::Title::Show(show)) = &title.title else {
            panic!("a Shows Fulfiller was told to stop monitoring {title:?}");
        };
        self.stopped_monitoring_shows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((**show).clone());
        let refuses = self.refuses.load(Ordering::Relaxed);
        Box::pin(async move {
            if refuses {
                return Err("Sonarr is unreachable".into());
            }
            Ok(())
        })
    }

    fn progress<'a>(
        &'a self,
        _references: &'a [String],
    ) -> BoxFuture<'a, Result<HashMap<String, Vec<Download>>, PluginError>> {
        Box::pin(async { Ok(HashMap::new()) })
    }
}

/// Offers the profiles a test set for `quality_profile`, unless told to fail.
impl SettingsCheck for FakeFulfiller {
    fn check<'a>(&'a self, _settings: &'a Settings) -> BoxFuture<'a, Result<Choices, PluginError>> {
        let fails = self.check_fails.load(Ordering::Relaxed);
        let profiles = self
            .profiles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Box::pin(async move {
            if fails {
                return Err("the service is unreachable".into());
            }
            let choices = profiles.into_iter().map(|profile| Choice {
                value: profile.clone(),
                label: profile,
            });
            Ok(Choices::from([(
                "quality_profile".to_owned(),
                choices.collect(),
            )]))
        })
    }
}

/// A test Plugin providing a [`FakeFulfiller`] for Shows.
struct ShowsFulfillerPlugin(FakeFulfiller);

impl Plugin for ShowsFulfillerPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Fulfiller<Shows>>()],
            ..Manifest::new(PluginId::from_static(SHOWS_FULFILLER))
        }
    }

    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        Some(&self.0)
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, Arc::new(self.0.clone()));
    }
}

/// A test Plugin providing a [`FakeFulfiller`] for Movies.
struct FulfillerPlugin(FakeFulfiller);

impl Plugin for FulfillerPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Fulfiller<Movies>>()],
            ..Manifest::new(PluginId::from_static(MOVIE_FULFILLER))
        }
    }

    fn settings_check(&self) -> Option<&dyn SettingsCheck> {
        Some(&self.0)
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(self.0.clone()));
    }
}
