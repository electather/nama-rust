//! The Sonarr Installed Plugin: hands Approved Season Requests to Sonarr (ADR 0003), says how
//! their downloads are progressing, and has Sonarr stop monitoring deleted Episodes (ADR 0038).
//! The Server builds it for wasm32 and bundles it in its binary.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::{collections::HashMap, time::Duration};

    use nama_pdk::{
        Buffa, CheckedSettings, Download, FnResult, Fulfilled, Fulfiller, Manifest, Request,
        StopMonitoring, TitleId, TryAgainIn,
        extism_pdk::{self, Error},
        nama_plugin_proto::nama::plugin::v1::{
            EpisodeNumbers, RequestedSeason, request::MovieOrSeason, show_deleted::Deleted,
            stop_monitoring::Title,
        },
        plugin_fn, services, title_id,
    };
    use nama_servarr::{QualityProfile, Queued, Servarr};
    use serde::{Deserialize, Serialize};

    /// How long the Kernel waits before asking again while Sonarr refreshes a Show.
    const REFRESH_RETRY: Duration = Duration::from_secs(5);

    /// The code Sonarr gives when it refuses to add a Show because it already has it.
    const SERIES_EXISTS: &str = "SeriesExistsValidator";

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(Manifest {
            logo_svg: include_str!("../logo.svg").to_owned(),
            points: vec![nama_pdk::point(services::SHOW_FULFILLER)],
            show_id_namespaces: vec!["tvdb".to_owned()],
            settings: nama_servarr::settings("Sonarr", "Show", 8989),
            name: "Sonarr".to_owned(),
            description: "Sends Approved Season Requests to Sonarr to download.".to_owned(),
            ..nama_pdk::manifest("sonarr", env!("CARGO_PKG_VERSION"))
        }))
    }

    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        Ok(Buffa(Sonarr::from_config()?.api.checked_settings()?))
    }

    /// Has Sonarr monitor and search for each Season, adding its Show first if Sonarr doesn't have
    /// it, and follows a Show by having Sonarr monitor its new Seasons. Its reference is
    /// `<the Show's TVDB id>/<the Season number>`. A Show Sonarr already has keeps its own
    /// profile, as Sonarr keeps one per series.
    struct SonarrFulfiller;

    impl Fulfiller for SonarrFulfiller {
        fn fulfill(request: Request) -> Result<Fulfilled, Error> {
            let tvdb_id = show_tvdb_id(&request)?;
            let season_number = asked_season(&request)?.season_number;
            let asked = nama_servarr::quality_profile(&request)?;
            let sonarr = Sonarr::from_config()?;
            let profiles = sonarr.api.quality_profiles()?;
            let had = match sonarr.find(tvdb_id)? {
                None => sonarr.add_asked(&request, asked, &profiles)?,
                had => had,
            };
            let Some(show) = had else {
                return Ok(fulfilled(tvdb_id, season_number, asked, profiles));
            };
            sonarr.not_refreshing(show.id)?;
            if request.followed_show_id.is_some() {
                sonarr.monitor_new_seasons(show.id, true)?;
            }
            sonarr.monitor_and_search(show.id, season_number)?;
            let kept = show.quality_profile_id;
            Ok(fulfilled(tvdb_id, season_number, kept, profiles))
        }

        fn unfollow(request: Request) -> Result<(), Error> {
            let sonarr = Sonarr::from_config()?;
            // A Show Sonarr doesn't have, it isn't following.
            let Some(show) = sonarr.find(show_tvdb_id(&request)?)? else {
                return Ok(());
            };
            // A refresh saves the whole Show as it read it, so would undo the change too.
            sonarr.not_refreshing(show.id)?;
            sonarr.monitor_new_seasons(show.id, false)
        }

        /// Unmonitors the deleted Episodes, or Seasons and their Episodes. Sonarr keeps the Show
        /// monitored, and its new Seasons as they were, so a Followed Show still gets them.
        fn stop_monitoring(title: StopMonitoring) -> Result<(), Error> {
            let Some(Title::Show(show_deleted)) = title.title else {
                return Err(Error::msg("Sonarr stops monitoring only Shows"));
            };
            let tvdb_id = needed_tvdb_id(&show_deleted.show_ids)?;
            let sonarr = Sonarr::from_config()?;
            let Some(show) = sonarr.find(tvdb_id)? else {
                return Ok(());
            };
            // A refresh saves the whole Show as it read it, so would undo the change too.
            sonarr.not_refreshing(show.id)?;
            let episodes: Vec<Episode> =
                sonarr.api.get(&format!("episode?seriesId={}", show.id))?;
            let (seasons, episodes) = match show_deleted.deleted {
                Some(Deleted::EveryCurrentSeason(_)) => {
                    let seasons = show.seasons.iter().map(|season| season.season_number);
                    (seasons.collect(), episodes)
                }
                Some(Deleted::SeasonNumber(number)) => {
                    let of_season = |episode: &Episode| episode.season_number == number;
                    (
                        vec![number],
                        episodes.into_iter().filter(of_season).collect(),
                    )
                }
                Some(Deleted::Episodes(named)) => {
                    let named = |episode: &Episode| episode.is_in(&named);
                    (Vec::new(), episodes.into_iter().filter(named).collect())
                }
                None => return Err(Error::msg("Sonarr was told to stop monitoring nothing")),
            };
            sonarr.unmonitor(show.id, seasons, episodes)
        }

        /// Reads Sonarr's whole queue in one call, whatever the number of `references`. A Season
        /// that isn't in it has no downloads, and Availability comes from the Library, not here
        /// (ADR 0012).
        fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            Sonarr::from_config()?
                .api
                .progress::<QueuedSeason>(&references)
        }
    }

    nama_pdk::fulfiller!(SonarrFulfiller);

    /// Sonarr's API, as the settings say where it is and its key.
    struct Sonarr {
        api: Servarr,
    }

    impl Sonarr {
        fn from_config() -> Result<Self, Error> {
            let api = Servarr::from_settings("Sonarr")?;
            Ok(Self { api })
        }

        /// The Show with the TVDB id `tvdb_id`, if Sonarr has it.
        fn find(&self, tvdb_id: i64) -> Result<Option<Added>, Error> {
            let had: Vec<Added> = self.api.get(&format!("series?tvdbId={tvdb_id}"))?;
            Ok(had.into_iter().next())
        }

        /// The Show with the TVDB id `tvdb_id` as Sonarr would add it.
        fn look_up(&self, tvdb_id: i64) -> Result<Show, Error> {
            let found: Vec<Show> = self
                .api
                .get(&format!("series/lookup?term=tvdb:{tvdb_id}"))?;
            found.into_iter().next().ok_or_else(|| {
                Error::msg(format!(
                    "Sonarr couldn't find the Show by its TVDB id, {tvdb_id}"
                ))
            })
        }

        /// Adds the Show `request` asks for at the quality profile `asked`, one of `profiles`, as
        /// [`Self::add`] does. Returns `None` once it is added, or the Show as another hand-off
        /// added it meanwhile.
        fn add_asked(
            &self,
            request: &Request,
            asked: i64,
            profiles: &[QualityProfile],
        ) -> Result<Option<Added>, Error> {
            // Checked first, as Sonarr may add a Show with a profile it no longer has.
            if !profiles.iter().any(|profile| profile.id == asked) {
                return Err(self.api.no_such_profile(request, asked));
            }
            let tvdb_id = show_tvdb_id(request)?;
            let follow = request.followed_show_id.is_some();
            let show = self.look_up(tvdb_id)?;
            if self.add(show, asked_season(request)?.season_number, follow, asked)? {
                return Ok(None);
            }
            // Another hand-off for the Show added it since `find`.
            self.find(tvdb_id)?
                .ok_or_else(|| Error::msg("Sonarr said it has the Show, then that it doesn't"))
                .map(Some)
        }

        /// Adds `show` at the quality profile `profile` with only the Season `season_number`
        /// monitored, and its new Seasons too if `follow`, and has Sonarr search for it. Returns
        /// `false` if Sonarr already has the Show.
        fn add(
            &self,
            show: Show,
            season_number: u32,
            follow: bool,
            profile: i64,
        ) -> Result<bool, Error> {
            let seasons = show
                .seasons
                .into_iter()
                .map(|season| Season {
                    monitored: season.season_number == season_number,
                    ..season
                })
                .collect();
            let new_show = NewShow {
                tvdb_id: show.tvdb_id,
                title: show.title,
                quality_profile_id: profile,
                root_folder_path: nama_servarr::root_folder()?,
                monitored: true,
                monitor_new_items: new_items(follow),
                season_folder: true,
                seasons,
                add_options: AddOptions {
                    monitor: "skip",
                    search_for_missing_episodes: true,
                },
            };
            self.api.add("series", new_show, SERIES_EXISTS)
        }

        /// Fails with [`TryAgainIn`] while Sonarr has a refresh of the Show `show_id` queued or
        /// running, so the Kernel asks again later. A refresh reads the Show's Seasons, fetches
        /// it from TVDB, then saves the Seasons as it read them, so a Season monitored meanwhile
        /// is dropped. Sonarr starts one as it adds a Show, so a Season handed off right after
        /// another one added its Show would be.
        fn not_refreshing(&self, show_id: i64) -> Result<(), Error> {
            if self.refreshing(show_id)? {
                return Err(TryAgainIn(REFRESH_RETRY).into());
            }
            Ok(())
        }

        /// Whether Sonarr has a refresh of the Show `show_id` queued or running.
        fn refreshing(&self, show_id: i64) -> Result<bool, Error> {
            let commands: Vec<Command> = self.api.get("command")?;
            Ok(commands.iter().any(|command| command.refreshes(show_id)))
        }

        /// Has Sonarr monitor the new Seasons of the Show it has as `show_id`, or stop to.
        fn monitor_new_seasons(&self, show_id: i64, monitored: bool) -> Result<(), Error> {
            let editor = SeriesEditor {
                series_ids: [show_id],
                monitor_new_items: new_items(monitored),
            };
            self.api.change("PUT", "series/editor", editor)
        }

        /// Unmonitors the Seasons `seasons` and the Episodes `episodes` of the Show Sonarr has as
        /// `show_id`, leaving the Show and its other Seasons as they are.
        fn unmonitor(
            &self,
            show_id: i64,
            seasons: Vec<u32>,
            episodes: Vec<Episode>,
        ) -> Result<(), Error> {
            if !seasons.is_empty() {
                let seasons = (seasons.into_iter())
                    .map(|season_number| Season {
                        season_number,
                        monitored: false,
                    })
                    .collect();
                let pass = SeasonPass {
                    series: [ShowSeasons {
                        id: show_id,
                        monitored: None,
                        seasons,
                    }],
                };
                self.api.change("POST", "seasonpass", pass)?;
            }
            if episodes.is_empty() {
                return Ok(());
            }
            let monitor = EpisodesMonitored {
                episode_ids: episodes.into_iter().map(|episode| episode.id).collect(),
                monitored: false,
            };
            self.api.change("PUT", "episode/monitor", monitor)
        }

        /// Monitors the Season `season_number` of the Show Sonarr has as `show_id`, and its
        /// Episodes, and searches for them. Each request names only that Season or its Episodes,
        /// so another hand-off changing the Show's other Seasons meanwhile keeps its change.
        fn monitor_and_search(&self, show_id: i64, season_number: u32) -> Result<(), Error> {
            let season = Season {
                season_number,
                monitored: true,
            };
            let pass = SeasonPass {
                series: [ShowSeasons {
                    id: show_id,
                    // Sonarr searches for nothing of a Show it doesn't monitor.
                    monitored: Some(true),
                    seasons: vec![season],
                }],
            };
            self.api.change("POST", "seasonpass", pass)?;
            let episodes: Vec<Episode> = self.api.get(&format!(
                "episode?seriesId={show_id}&seasonNumber={season_number}"
            ))?;
            let monitor = EpisodesMonitored {
                episode_ids: episodes.into_iter().map(|episode| episode.id).collect(),
                monitored: true,
            };
            self.api.change("PUT", "episode/monitor", monitor)?;
            let search = SeasonSearch {
                name: "SeasonSearch",
                series_id: show_id,
                season_number,
            };
            self.api.change("POST", "command", search)
        }
    }

    /// What a hand-off of the Season `season_number` of the Show `tvdb_id` reports: its reference,
    /// and the profile `profile_id` Sonarr has the Show at, named as in `profiles`.
    fn fulfilled(
        tvdb_id: i64,
        season_number: u32,
        profile_id: i64,
        profiles: Vec<QualityProfile>,
    ) -> Fulfilled {
        let reference = season_reference(tvdb_id, season_number);
        nama_servarr::fulfilled(reference, Some(profile_id), profiles)
    }

    /// The Season `request` asks for. The Server hands Sonarr only Season Requests.
    fn asked_season(request: &Request) -> Result<&RequestedSeason, Error> {
        match &request.movie_or_season {
            Some(MovieOrSeason::Season(season)) => Ok(season),
            _ => Err(Error::msg("Sonarr takes only Season Requests")),
        }
    }

    fn show_tvdb_id(request: &Request) -> Result<i64, Error> {
        needed_tvdb_id(&asked_season(request)?.show_ids)
    }

    /// The TVDB id among a Show's `ids`, as the number Sonarr's API finds it by, or why there is
    /// none.
    fn needed_tvdb_id(ids: &[TitleId]) -> Result<i64, Error> {
        title_id(ids, "tvdb")
            .and_then(|tvdb| tvdb.parse().ok())
            .ok_or_else(|| Error::msg("Sonarr needs the Show's TVDB id, and none is known for it"))
    }

    /// Sonarr's `monitorNewItems` for a Show whose new Seasons it monitors if `monitored`.
    fn new_items(monitored: bool) -> &'static str {
        if monitored { "all" } else { "none" }
    }

    /// What an entry in Sonarr's queue is for: one Episode of a Season.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QueuedSeason {
        series: Option<ShowIds>,
        season_number: Option<u32>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ShowIds {
        tvdb_id: i64,
    }

    impl Queued for QueuedSeason {
        const INCLUDE: &'static str = "includeSeries";

        /// The reference `fulfill` gave its Season.
        fn reference(&self) -> Option<String> {
            Some(season_reference(
                self.series.as_ref()?.tvdb_id,
                self.season_number?,
            ))
        }
    }

    /// The reference `fulfill` gives the Season `season_number` of the Show `tvdb_id`.
    fn season_reference(tvdb_id: i64, season_number: u32) -> String {
        format!("{tvdb_id}/{season_number}")
    }

    /// A Show as Sonarr's lookup gives it.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Show {
        tvdb_id: i64,
        title: String,
        seasons: Vec<Season>,
    }

    /// A Show Sonarr has.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Added {
        /// Sonarr's own id for it.
        id: i64,
        quality_profile_id: i64,
        seasons: Vec<Season>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Season {
        season_number: u32,
        monitored: bool,
    }

    /// What `POST /api/v3/series` takes to add a Show.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct NewShow {
        tvdb_id: i64,
        title: String,
        quality_profile_id: i64,
        root_folder_path: String,
        monitored: bool,
        monitor_new_items: &'static str,
        season_folder: bool,
        seasons: Vec<Season>,
        add_options: AddOptions,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AddOptions {
        /// `skip` keeps the Seasons' monitoring as sent. Left out, Sonarr rewrites it after its
        /// first scan, from the Seasons as it read them, and so drops a later hand-off's Season.
        monitor: &'static str,
        /// Searches the monitored Episodes, which are only the requested Season's.
        search_for_missing_episodes: bool,
    }

    /// What `PUT /api/v3/series/editor` takes. Sonarr changes only the fields it is sent.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SeriesEditor {
        series_ids: [i64; 1],
        monitor_new_items: &'static str,
    }

    /// What `POST /api/v3/seasonpass` takes. Sonarr changes only the Seasons it names.
    #[derive(Serialize)]
    struct SeasonPass {
        series: [ShowSeasons; 1],
    }

    #[derive(Serialize)]
    struct ShowSeasons {
        id: i64,
        /// Whether Sonarr monitors the Show. Left out, it stays as it is.
        #[serde(skip_serializing_if = "Option::is_none")]
        monitored: Option<bool>,
        seasons: Vec<Season>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Episode {
        id: i64,
        season_number: u32,
        #[serde(rename = "episodeNumber")]
        number: u32,
    }

    impl Episode {
        fn is_in(&self, named: &EpisodeNumbers) -> bool {
            (named.episodes.iter()).any(|named| {
                named.season_number == self.season_number && named.episode_number == self.number
            })
        }
    }

    /// What `PUT /api/v3/episode/monitor` takes. The season pass leaves Episodes as they are.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct EpisodesMonitored {
        episode_ids: Vec<i64>,
        monitored: bool,
    }

    /// A command Sonarr has queued, is running or ran lately, as `GET /api/v3/command` lists it.
    #[derive(Deserialize)]
    struct Command {
        name: String,
        status: String,
        #[serde(default)]
        body: CommandBody,
    }

    impl Command {
        /// Whether it is a refresh of the Show `show_id` that hasn't finished.
        fn refreshes(&self, show_id: i64) -> bool {
            self.name == "RefreshSeries"
                && matches!(self.status.as_str(), "queued" | "started")
                && self.body.series_ids.contains(&show_id)
        }
    }

    #[derive(Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct CommandBody {
        /// The Shows a `RefreshSeries` refreshes; none for one that refreshes them all.
        #[serde(default)]
        series_ids: Vec<i64>,
    }

    /// The command that searches for a Season's monitored Episodes.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SeasonSearch {
        name: &'static str,
        series_id: i64,
        season_number: u32,
    }

    #[cfg(test)]
    mod tests {
        use std::time::Duration;

        use nama_pdk::{
            Fulfiller, Request, Response, TitleId, TryAgainIn,
            extism_pdk::json::{Value, json},
            nama_plugin_proto::nama::plugin::v1::RequestedSeason,
            testing::{Kernel, with},
        };

        use super::{SonarrFulfiller, needed_tvdb_id};

        const DARK_TVDB: i64 = 81189;
        /// Sonarr's own id for Dark, once it has it.
        const DARK_SONARR_ID: i64 = 7;

        /// Sonarr's settings, answering each request with the next of `answers`, a status and a
        /// JSON body.
        fn sonarr(answers: Vec<(u16, Value)>) -> Kernel {
            let mut answers = answers.into_iter();
            Kernel::new()
                .setting("url", "http://sonarr.test")
                .setting("api_key", "key")
                .setting("quality_profile", "4")
                .setting("root_folder", "/tv")
                .http(move |_request, _body| {
                    let (status, body) = answers
                        .next()
                        .expect("Sonarr is asked no more than scripted");
                    Ok(Response::new(status, body.to_string()))
                })
        }

        /// What the Plugin asked Sonarr, as `<method> <API path>`.
        fn asked(kernel: &Kernel) -> Vec<String> {
            (kernel.sent().iter())
                .map(|sent| {
                    let method = sent.request.method.as_deref().unwrap_or("GET");
                    let path = sent
                        .request
                        .url
                        .trim_start_matches("http://sonarr.test/api/v3/");
                    format!("{method} {path}")
                })
                .collect()
        }

        fn json_body(body: &[u8]) -> Value {
            nama_pdk::extism_pdk::json::from_slice(body).unwrap()
        }

        /// An Approved Request for Season 3 of Dark.
        fn dark_season_3() -> Request {
            let tvdb = id("tvdb", &DARK_TVDB.to_string());
            Request {
                movie_or_season: RequestedSeason {
                    season_number: 3,
                    show_ids: vec![tvdb],
                    ..RequestedSeason::default()
                }
                .into(),
                ..Request::default()
            }
        }

        fn profiles() -> (u16, Value) {
            (
                200,
                json!([{"id": 1, "name": "Any"}, {"id": 4, "name": "HD-1080p"}]),
            )
        }

        /// Dark as Sonarr has it, at the profile Any.
        fn dark_had() -> (u16, Value) {
            let seasons = json!([{"seasonNumber": 1, "monitored": true}]);
            (
                200,
                json!([{"id": DARK_SONARR_ID, "qualityProfileId": 1, "seasons": seasons}]),
            )
        }

        #[test]
        fn a_show_another_hand_off_added_meanwhile_gets_the_season_monitored_at_its_profile() {
            let lookup = json!([{"tvdbId": DARK_TVDB, "title": "Dark", "seasons": [
                {"seasonNumber": 1, "monitored": false},
                {"seasonNumber": 3, "monitored": false}
            ]}]);
            let exists =
                json!([{"errorMessage": "It exists", "errorCode": "SeriesExistsValidator"}]);
            let episodes = json!([{"id": 301, "seasonNumber": 3, "episodeNumber": 1}]);
            let mut kernel = sonarr(vec![
                profiles(),
                (200, json!([])),
                (200, lookup),
                (400, exists),
                dark_had(),
                (200, json!([])),
                (200, json!({})),
                (200, episodes),
                (202, json!({})),
                (201, json!({})),
            ]);

            let fulfilled =
                with(&mut kernel, || SonarrFulfiller::fulfill(dark_season_3())).unwrap();

            assert_eq!(
                asked(&kernel),
                [
                    "GET qualityprofile",
                    "GET series?tvdbId=81189",
                    "GET series/lookup?term=tvdb:81189",
                    "POST series",
                    "GET series?tvdbId=81189",
                    "GET command",
                    "POST seasonpass",
                    "GET episode?seriesId=7&seasonNumber=3",
                    "PUT episode/monitor",
                    "POST command",
                ]
            );
            let monitored: Value = (kernel.sent().iter())
                .find(|sent| sent.request.url.ends_with("/seasonpass"))
                .map(|sent| json_body(&sent.body))
                .unwrap();
            assert_eq!(
                monitored,
                json!({"series": [{"id": DARK_SONARR_ID, "monitored": true, "seasons": [
                    {"seasonNumber": 3, "monitored": true}
                ]}]})
            );
            assert_eq!(
                (
                    fulfilled.reference.as_str(),
                    fulfilled.profile_name.as_str()
                ),
                ("81189/3", "Any")
            );
        }

        #[test]
        fn a_season_of_a_show_sonarr_is_refreshing_is_tried_again_in_5_seconds_changing_nothing() {
            let refreshing = json!([{"name": "RefreshSeries", "status": "started",
                "body": {"seriesIds": [DARK_SONARR_ID]}}]);
            let mut kernel = sonarr(vec![profiles(), dark_had(), (200, refreshing)]);

            let error =
                with(&mut kernel, || SonarrFulfiller::fulfill(dark_season_3())).unwrap_err();

            assert_eq!(
                error.downcast_ref::<TryAgainIn>(),
                Some(&TryAgainIn(Duration::from_secs(5)))
            );
            assert_eq!(
                asked(&kernel),
                [
                    "GET qualityprofile",
                    "GET series?tvdbId=81189",
                    "GET command"
                ]
            );
        }

        fn id(namespace: &str, value: &str) -> TitleId {
            TitleId {
                namespace: namespace.to_owned(),
                value: value.to_owned(),
                ..TitleId::default()
            }
        }

        #[test]
        fn a_show_without_a_tvdb_id_is_refused() {
            let error = needed_tvdb_id(&[id("tmdb", "1396")]).unwrap_err();

            assert_eq!(
                error.to_string(),
                "Sonarr needs the Show's TVDB id, and none is known for it"
            );
        }

        #[test]
        fn a_show_s_tvdb_id_is_its_number() {
            let ids = [id("tmdb", "1396"), id("tvdb", "81189")];

            assert_eq!(needed_tvdb_id(&ids).unwrap(), 81189);
        }

        #[test]
        fn a_show_whose_tvdb_id_is_not_a_number_is_refused() {
            let error = needed_tvdb_id(&[id("tvdb", "breaking-bad")]).unwrap_err();

            assert_eq!(
                error.to_string(),
                "Sonarr needs the Show's TVDB id, and none is known for it"
            );
        }
    }
}
