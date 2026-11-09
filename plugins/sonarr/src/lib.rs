//! The Sonarr Installed Plugin: hands Approved Season Requests to Sonarr (ADR 0003), and says how
//! their downloads are progressing. The Server builds it for wasm32 and bundles it in its binary.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::collections::{HashMap, HashSet};

    use nama_pdk::{
        Buffa, CheckedSettings, Download, FnResult, Fulfiller, Manifest, Request,
        extism_pdk::{self, Error, HttpRequest, HttpResponse, Json, ToMemory, config},
        nama_proto::nama::{
            plugin::v1::{ProvidePoint, SettingField, SettingKind},
            v1::DownloadStatus,
        },
        plugin_fn,
    };
    use serde::{Deserialize, Serialize, de::DeserializeOwned};

    const URL: &str = "url";
    const API_KEY: &str = "api_key";
    const QUALITY_PROFILE: &str = "quality_profile";
    const ROOT_FOLDER: &str = "root_folder";

    /// How long, in seconds by Sonarr's `Date` header, a hand-off waits for Sonarr to finish
    /// adding its Show. That header counts whole seconds, so the wait lasts between one and two
    /// seconds, well inside the 5 s a call may take.
    const ADD_WAIT_SECONDS: i64 = 2;
    const SECONDS_A_DAY: i64 = 24 * 60 * 60;

    /// The code Sonarr gives when it refuses to add a Show because it already has it.
    const SERIES_EXISTS: &str = "SeriesExistsValidator";

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        let setting = |key: &str, label: &str, description: &str, kind: SettingKind| SettingField {
            key: key.to_owned(),
            label: label.to_owned(),
            description: description.to_owned(),
            kind: kind.into(),
            required: true,
            ..SettingField::default()
        };
        Ok(Buffa(Manifest {
            providers: vec![ProvidePoint::PROVIDE_POINT_SHOW_FULFILLER.into()],
            settings: vec![
                setting(
                    URL,
                    "URL",
                    "Where Sonarr is, such as http://localhost:8989.",
                    SettingKind::SETTING_KIND_URL,
                ),
                setting(
                    API_KEY,
                    "API key",
                    "Sonarr's API key, from Settings > General in Sonarr.",
                    SettingKind::SETTING_KIND_SECRET,
                ),
                setting(
                    QUALITY_PROFILE,
                    "Quality profile",
                    "The quality Sonarr downloads each Show in.",
                    SettingKind::SETTING_KIND_CHOICE,
                ),
                setting(
                    ROOT_FOLDER,
                    "Root folder",
                    "The folder Sonarr puts each Show in.",
                    SettingKind::SETTING_KIND_CHOICE,
                ),
            ],
            name: "Sonarr".to_owned(),
            description: "Sends Approved Season Requests to Sonarr to download.".to_owned(),
            ..nama_pdk::manifest("sonarr", env!("CARGO_PKG_VERSION"))
        }))
    }

    // Offers Sonarr's quality profiles, by id, and its root folders, by path.
    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        let sonarr = Sonarr::from_config()?;
        let profiles: Vec<QualityProfile> = sonarr.get("qualityprofile")?;
        let folders: Vec<RootFolder> = sonarr.get("rootfolder")?;
        let profiles: Vec<_> = profiles
            .into_iter()
            .map(|profile| (profile.id.to_string(), profile.name))
            .collect();
        let folders = folders
            .into_iter()
            .map(|folder| (folder.path.clone(), folder.path))
            .collect();
        Ok(Buffa(nama_pdk::checked_settings([
            (QUALITY_PROFILE, profiles),
            (ROOT_FOLDER, folders),
        ])))
    }

    /// Has Sonarr monitor and search for each Season, adding its Show first if Sonarr doesn't have
    /// it, and follows a Show by having Sonarr monitor its new Seasons. Its reference is
    /// `<the Show's TVDB id>/<the Season number>`.
    struct SonarrFulfiller;

    impl Fulfiller for SonarrFulfiller {
        fn fulfill(request: Request) -> Result<String, Error> {
            let tvdb_id = show_tvdb_id(&request)?;
            let season_number = request.season.season_number;
            let follow = request.followed_show_id.is_some();
            let reference = season_reference(tvdb_id, season_number);
            let sonarr = Sonarr::from_config()?;
            let id = match sonarr.find(tvdb_id)? {
                Some(id) => id,
                None if sonarr.add(sonarr.look_up(tvdb_id)?, season_number, follow)? => {
                    return Ok(reference);
                }
                // Another hand-off for the Show added it since `find`.
                None => sonarr.find(tvdb_id)?.ok_or_else(|| {
                    Error::msg("Sonarr said it has the Show, then that it doesn't")
                })?,
            };
            sonarr.wait_until_added(id)?;
            if follow {
                sonarr.monitor_new_seasons(id, true)?;
            }
            sonarr.monitor_and_search(id, season_number)?;
            Ok(reference)
        }

        fn unfollow(request: Request) -> Result<(), Error> {
            let sonarr = Sonarr::from_config()?;
            // A Show Sonarr doesn't have, it isn't following.
            let Some(id) = sonarr.find(show_tvdb_id(&request)?)? else {
                return Ok(());
            };
            // A refresh saves the whole Show as it read it, so would undo the change too.
            sonarr.wait_until_added(id)?;
            sonarr.monitor_new_seasons(id, false)
        }

        /// Reads Sonarr's whole queue in one call, whatever the number of `references`. A Season
        /// that isn't in it has no downloads, and Availability comes from the Library, not here
        /// (ADR 0012).
        fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            // ponytail: reads only the first 1000 entries, one per Episode; page through
            // `totalRecords` if a queue ever holds more.
            let queue: Queue =
                Sonarr::from_config()?.get("queue?pageSize=1000&includeSeries=true")?;
            let mut listed = HashSet::new();
            let in_batch = queue
                .records
                .into_iter()
                .filter_map(|entry| Some((entry.reference()?, entry)))
                .filter(|(reference, _)| references.contains(reference))
                // Sonarr lists a Season pack once for each of its Episodes.
                .filter(|(reference, entry)| {
                    let download = (entry.download_id.clone(), entry.title.clone());
                    listed.insert((reference.clone(), download))
                });
            let mut downloads: HashMap<String, Vec<Download>> = HashMap::new();
            for (reference, entry) in in_batch {
                downloads.entry(reference).or_default().push(entry.into());
            }
            Ok(downloads)
        }
    }

    nama_pdk::fulfiller!(SonarrFulfiller);

    /// Sonarr's API, as the settings say where it is and its key.
    struct Sonarr {
        url: String,
        api_key: String,
    }

    impl Sonarr {
        fn from_config() -> Result<Self, Error> {
            Ok(Self {
                url: setting(URL)?,
                api_key: setting(API_KEY)?,
            })
        }

        /// Sonarr's own id for the Show with the TVDB id `tvdb_id`, if Sonarr has it.
        fn find(&self, tvdb_id: i64) -> Result<Option<i64>, Error> {
            let had: Vec<Added> = self.get(&format!("series?tvdbId={tvdb_id}"))?;
            Ok(had.first().map(|show| show.id))
        }

        /// The Show with the TVDB id `tvdb_id` as Sonarr would add it.
        fn look_up(&self, tvdb_id: i64) -> Result<Show, Error> {
            let found: Vec<Show> = self.get(&format!("series/lookup?term=tvdb:{tvdb_id}"))?;
            found.into_iter().next().ok_or_else(|| {
                Error::msg(format!(
                    "Sonarr couldn't find the Show by its TVDB id, {tvdb_id}"
                ))
            })
        }

        /// Adds `show` with only the Season `season_number` monitored, and its new Seasons too if
        /// `follow`, and has Sonarr search for it. Returns `false` if Sonarr already has the Show.
        fn add(&self, show: Show, season_number: u32, follow: bool) -> Result<bool, Error> {
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
                quality_profile_id: setting(QUALITY_PROFILE)?
                    .parse()
                    .map_err(|err| Error::msg(format!("the quality profile isn't an id: {err}")))?,
                root_folder_path: setting(ROOT_FOLDER)?,
                monitored: true,
                monitor_new_items: new_items(follow),
                season_folder: true,
                seasons,
                add_options: AddOptions {
                    monitor: "skip",
                    search_for_missing_episodes: true,
                },
            };
            let response = send(&self.request("POST", "series"), Some(Json(new_show)))?;
            if already_has(&response) {
                return Ok(false);
            }
            accepted(response).map(|_| true)
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
            let response = self.get_response("command")?;
            let commands: Vec<Command> = parsed(&response)?;
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
            self.change("PUT", "series/editor", editor)
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
            self.change("POST", "seasonpass", pass)?;
            let episodes: Vec<Episode> = self.get(&format!(
                "episode?seriesId={show_id}&seasonNumber={season_number}"
            ))?;
            let monitor = EpisodesMonitored {
                episode_ids: episodes.into_iter().map(|episode| episode.id).collect(),
                monitored: true,
            };
            self.change("PUT", "episode/monitor", monitor)?;
            let search = SeasonSearch {
                name: "SeasonSearch",
                series_id: show_id,
                season_number,
            };
            self.change("POST", "command", search)
        }

        /// A `method` request for the API path `path`, such as `series`.
        fn request(&self, method: &str, path: &str) -> HttpRequest {
            let url = format!("{}/api/v3/{path}", self.url.trim_end_matches('/'));
            HttpRequest::new(url)
                .with_method(method)
                .with_header("X-Api-Key", &self.api_key)
                .with_header("Content-Type", "application/json")
        }

        /// What Sonarr answers at the API path `path`.
        fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
            parsed(&self.get_response(path)?)
        }

        /// Sonarr's response at the API path `path`, if it took the request.
        fn get_response(&self, path: &str) -> Result<HttpResponse, Error> {
            accepted(send(&self.request("GET", path), None::<()>)?)
        }

        /// Sends Sonarr the change `body` to the API path `path` with `method`, ignoring what it
        /// answers once it takes it.
        fn change(&self, method: &str, path: &str, body: impl Serialize) -> Result<(), Error> {
            accepted(send(&self.request(method, path), Some(Json(body)))?).map(drop)
        }
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

    fn send<T: ToMemory>(request: &HttpRequest, body: Option<T>) -> Result<HttpResponse, Error> {
        // Not naming the URL: a Request's reason reaches every Requester.
        nama_pdk::http_request(request, body, "Sonarr didn't answer")
    }

    /// The setting `key`, which the Server sets before every call but a settings check, and for
    /// that one too unless it is a Choice.
    fn setting(key: &str) -> Result<String, Error> {
        config::get(key)?.ok_or_else(|| Error::msg(format!("the setting {key} isn't set")))
    }

    /// Why a Season Request fails when Sonarr takes too long adding its Show.
    fn still_adding() -> Error {
        Error::msg(
            "Sonarr was still adding the Show, so it would have forgotten this Season. \
             Retry in a minute.",
        )
    }

    fn parsed<T: DeserializeOwned>(response: &HttpResponse) -> Result<T, Error> {
        response
            .json()
            .map_err(|err| Error::msg(format!("Sonarr's answer made no sense: {err}")))
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

    /// `response`, if Sonarr took the request, or else why it refused.
    fn accepted(response: HttpResponse) -> Result<HttpResponse, Error> {
        if (200..300).contains(&response.status_code()) {
            Ok(response)
        } else {
            Err(refused(&response))
        }
    }

    fn already_has(response: &HttpResponse) -> bool {
        failures(response)
            .iter()
            .any(|failure| failure.error_code.as_deref() == Some(SERIES_EXISTS))
    }

    /// Why Sonarr refused a request, in words a User understands.
    fn refused(response: &HttpResponse) -> Error {
        let messages: Vec<_> = failures(response)
            .into_iter()
            .map(|failure| failure.error_message)
            .collect();
        let reason = match response.status_code() {
            401 => "Sonarr refused the API key".to_owned(),
            400 if !messages.is_empty() => format!("Sonarr said: {}", messages.join(" ")),
            status => format!("Sonarr failed with HTTP status {status}"),
        };
        Error::msg(reason)
    }

    /// The validation failures in the body of a `400 Bad Request` from Sonarr, if it is one.
    fn failures(response: &HttpResponse) -> Vec<ValidationFailure> {
        if response.status_code() == 400 {
            response.json().unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// A page of `GET /api/v3/queue`.
    #[derive(Deserialize)]
    struct Queue {
        records: Vec<QueueEntry>,
    }

    /// One Episode of a download in Sonarr's queue, so a Season pack has an entry for each.
    /// Sonarr may send `null` for those that are `Option`s.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QueueEntry {
        /// Only there with `includeSeries=true`.
        series: Option<QueuedShow>,
        season_number: Option<u32>,
        /// The download client's id for the download, shared by a pack's entries.
        download_id: Option<String>,
        title: Option<String>,
        size: f64,
        sizeleft: f64,
        /// How long it has left, as a .NET `TimeSpan`.
        timeleft: Option<String>,
        /// Where the download client has it, such as `downloading` or `completed`.
        status: String,
        /// `ok`, `warning` or `error`.
        tracked_download_status: Option<String>,
        /// Where Sonarr has it, such as `downloading`, `importBlocked` or `importing`.
        tracked_download_state: Option<String>,
        status_messages: Option<Vec<StatusMessage>>,
        error_message: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QueuedShow {
        tvdb_id: i64,
    }

    #[derive(Deserialize)]
    struct StatusMessage {
        messages: Option<Vec<String>>,
    }

    impl From<QueueEntry> for Download {
        fn from(entry: QueueEntry) -> Self {
            let status = entry.status();
            Self {
                percent: entry.percent(),
                size_bytes: entry.size_bytes(),
                // Sonarr gives a queued download's time left as zero.
                eta_seconds: (status == DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING)
                    .then(|| entry.timeleft.as_deref().and_then(seconds_of))
                    .flatten(),
                attention_reason: (status == DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION)
                    .then(|| entry.attention_reason())
                    .flatten(),
                status: status.into(),
                title: entry.title.unwrap_or_default(),
                ..Self::default()
            }
        }
    }

    impl QueueEntry {
        /// The reference `fulfill` gave its Season.
        fn reference(&self) -> Option<String> {
            Some(season_reference(
                self.series.as_ref()?.tvdb_id,
                self.season_number?,
            ))
        }

        fn status(&self) -> DownloadStatus {
            let tracked_state = self.tracked_download_state.as_deref();
            if self.needs_attention() {
                DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION
            } else if self.status == "completed"
                || matches!(
                    tracked_state,
                    Some("importPending" | "importing" | "imported")
                )
            {
                DownloadStatus::DOWNLOAD_STATUS_IMPORTING
            } else if self.status == "downloading" {
                DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING
            } else {
                DownloadStatus::DOWNLOAD_STATUS_QUEUED
            }
        }

        /// Whether it waits on a person, rather than on Sonarr or its download client.
        fn needs_attention(&self) -> bool {
            matches!(
                self.tracked_download_status.as_deref(),
                Some("warning" | "error")
            ) || matches!(
                self.status.as_str(),
                "failed" | "warning" | "downloadClientUnavailable"
            ) || matches!(
                self.tracked_download_state.as_deref(),
                Some("importBlocked" | "failedPending" | "failed")
            )
        }

        /// Sonarr's messages about it, if it has any.
        fn attention_reason(&self) -> Option<String> {
            let mut messages: Vec<&str> = self
                .status_messages
                .iter()
                .flatten()
                .flat_map(|status| status.messages.iter().flatten())
                .map(String::as_str)
                .collect();
            messages.extend(self.error_message.as_deref());
            (!messages.is_empty()).then(|| format!("Sonarr said: {}", messages.join(" ")))
        }

        #[expect(
            clippy::cast_possible_truncation,
            reason = "a percent needs no more than f32's precision"
        )]
        fn percent(&self) -> f32 {
            if self.size > 0.0 {
                ((self.size - self.sizeleft) / self.size * 100.0).clamp(0.0, 100.0) as f32
            } else {
                0.0
            }
        }

        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "Sonarr counts bytes in a double; only a positive size is kept"
        )]
        fn size_bytes(&self) -> Option<u64> {
            (self.size > 0.0).then_some(self.size as u64)
        }
    }

    /// The reference `fulfill` gives the Season `season_number` of the Show `tvdb_id`.
    fn season_reference(tvdb_id: i64, season_number: u32) -> String {
        format!("{tvdb_id}/{season_number}")
    }

    /// The seconds in a .NET `TimeSpan` such as `1.02:03:04`, dropping any fraction of a second.
    fn seconds_of(timespan: &str) -> Option<u64> {
        let mut parts = timespan.split(':');
        let (hours, minutes, seconds) = (parts.next()?, parts.next()?, parts.next()?);
        let (days, hours) = hours.split_once('.').unwrap_or(("0", hours));
        let seconds = seconds.split('.').next()?;
        let [days, hours, minutes, seconds] =
            [days, hours, minutes, seconds].map(|part| part.parse::<u64>().ok());
        Some(days? * 86_400 + hours? * 3_600 + minutes? * 60 + seconds?)
    }

    #[derive(Deserialize)]
    struct QualityProfile {
        id: i64,
        name: String,
    }

    #[derive(Deserialize)]
    struct RootFolder {
        path: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ValidationFailure {
        error_message: String,
        error_code: Option<String>,
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
    struct Added {
        /// Sonarr's own id for it.
        id: i64,
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
