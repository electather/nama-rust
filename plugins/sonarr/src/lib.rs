//! The Sonarr Installed Plugin: hands Approved Season Requests to Sonarr (ADR 0003), and says how
//! their downloads are progressing. The Server builds it for wasm32 and bundles it in its binary.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::collections::HashMap;

    use nama_pdk::{
        Buffa, CheckedSettings, Download, FnResult, Fulfilled, Fulfiller, Manifest, Request,
        extism_pdk::{self, Error, HttpResponse},
        nama_proto::nama::plugin::v1::ProvidePoint,
        plugin_fn,
    };
    use nama_servarr::{QualityProfile, Queued, Servarr};
    use serde::{Deserialize, Serialize};

    /// How long, in seconds by Sonarr's `Date` header, a hand-off waits for Sonarr to finish
    /// adding its Show. That header counts whole seconds, so the wait lasts between one and two
    /// seconds, well inside the 5 s a call may take.
    const ADD_WAIT_SECONDS: i64 = 2;
    const SECONDS_A_DAY: i64 = 24 * 60 * 60;

    /// The code Sonarr gives when it refuses to add a Show because it already has it.
    const SERIES_EXISTS: &str = "SeriesExistsValidator";

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(Manifest {
            logo_svg: include_str!("../logo.svg").to_owned(),
            providers: vec![ProvidePoint::PROVIDE_POINT_SHOW_FULFILLER.into()],
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
            let season_number = request.season.season_number;
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
            sonarr.wait_until_added(show.id)?;
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
            sonarr.wait_until_added(show.id)?;
            sonarr.monitor_new_seasons(show.id, false)
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
            if self.add(show, request.season.season_number, follow, asked)? {
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

        /// Returns once Sonarr has no refresh of the Show `show_id` queued or running, or fails
        /// after about [`ADD_WAIT_SECONDS`]. A refresh reads the Show's Seasons, fetches it from
        /// TVDB, then saves the Seasons as it read them, so a Season monitored meanwhile is
        /// dropped. Sonarr starts one as it adds a Show, so a Season handed off right after
        /// another one added its Show would be.
        fn wait_until_added(&self, show_id: i64) -> Result<(), Error> {
            let Some(first) = self.refreshing_at(show_id)? else {
                return Ok(());
            };
            while let Some(now) = self.refreshing_at(show_id)? {
                // The day may turn while it waits.
                let waited = (now - first).rem_euclid(SECONDS_A_DAY);
                (waited < ADD_WAIT_SECONDS)
                    .then_some(())
                    .ok_or_else(still_adding)?;
            }
            Ok(())
        }

        /// The second of the day by Sonarr's clock, if it has a refresh of the Show `show_id`
        /// queued or running.
        fn refreshing_at(&self, show_id: i64) -> Result<Option<i64>, Error> {
            let response = self.api.get_response("command")?;
            let commands: Vec<Command> = self.api.parsed(&response)?;
            if !commands.iter().any(|command| command.refreshes(show_id)) {
                return Ok(None);
            }
            // A Plugin has no clock, so a wait is timed by Sonarr's, and can't be without it.
            second_of_day(&response).map(Some).ok_or_else(still_adding)
        }

        /// Has Sonarr monitor the new Seasons of the Show it has as `show_id`, or stop to.
        fn monitor_new_seasons(&self, show_id: i64, monitored: bool) -> Result<(), Error> {
            let editor = SeriesEditor {
                series_ids: [show_id],
                monitor_new_items: new_items(monitored),
            };
            self.api.change("PUT", "series/editor", editor)
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
                    monitored: true,
                    seasons: [season],
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

    fn show_tvdb_id(request: &Request) -> Result<i64, Error> {
        request
            .season
            .show_tvdb_id
            .ok_or_else(|| Error::msg("Sonarr needs the Show's TVDB id, and none is known for it"))
    }

    /// Sonarr's `monitorNewItems` for a Show whose new Seasons it monitors if `monitored`.
    fn new_items(monitored: bool) -> &'static str {
        if monitored { "all" } else { "none" }
    }

    /// Why a Season Request fails when Sonarr takes too long adding its Show.
    fn still_adding() -> Error {
        Error::msg(
            "Sonarr was still adding the Show, so it would have forgotten this Season. \
             Retry in a minute.",
        )
    }

    /// The second of the day `response` was sent at, by its `Date` header.
    fn second_of_day(response: &HttpResponse) -> Option<i64> {
        // An HTTP date has a fixed shape, such as `Sun, 08 Nov 2026 12:00:03 GMT`.
        let time = response.header("date")?.get(17..25)?;
        let mut parts = time.split(':').map(str::parse::<i64>);
        let (Some(Ok(hours)), Some(Ok(minutes)), Some(Ok(seconds))) =
            (parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        Some((hours * 60 + minutes) * 60 + seconds)
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
        monitored: bool,
        seasons: [Season; 1],
    }

    #[derive(Deserialize)]
    struct Episode {
        id: i64,
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
}
