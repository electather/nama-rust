//! The Sonarr Installed Plugin: hands Approved Season Requests to Sonarr (ADR 0003). The Server
//! builds it for wasm32 and bundles it in its binary.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::collections::HashMap;

    use nama_pdk::{
        Buffa, CheckedSettings, Download, FnResult, Fulfiller, Manifest, Request,
        extism_pdk::{self, Error, HttpRequest, HttpResponse, Json, ToMemory, config},
        nama_proto::nama::plugin::v1::{ProvidePoint, SettingField, SettingKind},
        plugin_fn,
    };
    use serde::{Deserialize, Serialize, de::DeserializeOwned};

    const URL: &str = "url";
    const API_KEY: &str = "api_key";
    const QUALITY_PROFILE: &str = "quality_profile";
    const ROOT_FOLDER: &str = "root_folder";

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
    /// it. Its reference is `<the Show's TVDB id>/<the Season number>`.
    struct SonarrFulfiller;

    impl Fulfiller for SonarrFulfiller {
        fn fulfill(request: Request) -> Result<String, Error> {
            let tvdb_id = request.season.show_tvdb_id.ok_or_else(|| {
                Error::msg("Sonarr needs the Show's TVDB id, and none is known for it")
            })?;
            let number = request.season.season_number;
            let reference = format!("{tvdb_id}/{number}");
            let sonarr = Sonarr::from_config()?;
            let show = sonarr.look_up(tvdb_id)?;
            let id = match show.id() {
                Some(id) => id,
                None if sonarr.add(show, number)? => return Ok(reference),
                // Another hand-off for the Show added it since the lookup.
                None => sonarr.look_up(tvdb_id)?.id().ok_or_else(|| {
                    Error::msg("Sonarr said it has the Show, then that it doesn't")
                })?,
            };
            sonarr.monitor_and_search(id, number)?;
            Ok(reference)
        }

        // ponytail: says nothing is downloading until #146 reads Sonarr's queue.
        fn progress(_references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            Ok(HashMap::new())
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

        /// The Show Sonarr has with the TVDB id `tvdb_id`, or else the one it would add.
        fn look_up(&self, tvdb_id: i64) -> Result<Show, Error> {
            let found: Vec<Show> = self.get(&format!("series/lookup?term=tvdb:{tvdb_id}"))?;
            found.into_iter().next().ok_or_else(|| {
                Error::msg(format!(
                    "Sonarr couldn't find the Show by its TVDB id, {tvdb_id}"
                ))
            })
        }

        /// Adds `show` with only the Season `number` monitored, and has Sonarr search for it.
        /// Returns `false` if Sonarr already has the Show.
        fn add(&self, show: Show, number: u32) -> Result<bool, Error> {
            let seasons = show
                .seasons
                .into_iter()
                .map(|season| Season {
                    monitored: season.season_number == number,
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
                monitor_new_items: "none",
                season_folder: true,
                seasons,
                add_options: AddOptions {
                    search_for_missing_episodes: true,
                },
            };
            let response = send(&self.request("POST", "series"), Some(Json(new_show)))?;
            if is_success(&response) {
                Ok(true)
            } else if already_has(&response) {
                Ok(false)
            } else {
                Err(refused(&response))
            }
        }

        /// Monitors the Season `number` of the Show Sonarr has as `show_id`, and its Episodes,
        /// and searches for them. Each request names only that Season or its Episodes, so another
        /// hand-off changing the Show's other Seasons meanwhile keeps its change.
        fn monitor_and_search(&self, show_id: i64, number: u32) -> Result<(), Error> {
            let season = Season {
                season_number: number,
                monitored: true,
            };
            let pass = SeasonPass {
                series: [ShowSeasons {
                    id: show_id,
                    monitored: true,
                    seasons: [season],
                }],
            };
            self.send("POST", "seasonpass", pass)?;
            let episodes: Vec<Episode> =
                self.get(&format!("episode?seriesId={show_id}&seasonNumber={number}"))?;
            let monitor = EpisodesMonitored {
                episode_ids: episodes.into_iter().map(|episode| episode.id).collect(),
                monitored: true,
            };
            self.send("PUT", "episode/monitor", monitor)?;
            let search = SeasonSearch {
                name: "SeasonSearch",
                series_id: show_id,
                season_number: number,
            };
            self.send("POST", "command", search)
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
            let response = send(&self.request("GET", path), None::<()>)?;
            if !is_success(&response) {
                return Err(refused(&response));
            }
            response
                .json()
                .map_err(|err| Error::msg(format!("Sonarr's answer made no sense: {err}")))
        }

        /// Sends `body` to the API path `path` with `method`, ignoring what Sonarr answers.
        fn send(&self, method: &str, path: &str, body: impl Serialize) -> Result<(), Error> {
            let response = send(&self.request(method, path), Some(Json(body)))?;
            if is_success(&response) {
                Ok(())
            } else {
                Err(refused(&response))
            }
        }
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

    fn is_success(response: &HttpResponse) -> bool {
        (200..300).contains(&response.status_code())
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
        /// Sonarr's own id for it, which it has only once added.
        #[serde(default)]
        id: i64,
        tvdb_id: i64,
        title: String,
        seasons: Vec<Season>,
    }

    impl Show {
        /// Sonarr's own id for it, if Sonarr has it.
        fn id(&self) -> Option<i64> {
            (self.id > 0).then_some(self.id)
        }
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
        /// Following new Seasons is a Followed Show's job (#172), not a Season Request's.
        monitor_new_items: &'static str,
        season_folder: bool,
        seasons: Vec<Season>,
        add_options: AddOptions,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AddOptions {
        /// Searches the monitored Episodes, which are only the requested Season's.
        search_for_missing_episodes: bool,
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

    /// The command that searches for a Season's monitored Episodes.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SeasonSearch {
        name: &'static str,
        series_id: i64,
        season_number: u32,
    }
}
