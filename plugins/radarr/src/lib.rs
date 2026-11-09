//! The Radarr Installed Plugin: hands Approved Movie Requests to Radarr (ADR 0003), and says how
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

    /// The code Radarr gives when it refuses to add a Movie because it already has it.
    const MOVIE_EXISTS: &str = "MovieExistsValidator";

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
            logo_svg: include_str!("../logo.svg").to_owned(),
            providers: vec![ProvidePoint::PROVIDE_POINT_MOVIE_FULFILLER.into()],
            settings: vec![
                setting(
                    URL,
                    "URL",
                    "Where Radarr is, such as http://localhost:7878.",
                    SettingKind::SETTING_KIND_URL,
                ),
                setting(
                    API_KEY,
                    "API key",
                    "Radarr's API key, from Settings > General in Radarr.",
                    SettingKind::SETTING_KIND_SECRET,
                ),
                setting(
                    QUALITY_PROFILE,
                    "Quality profile",
                    "The quality Radarr downloads each Movie in.",
                    SettingKind::SETTING_KIND_CHOICE,
                ),
                setting(
                    ROOT_FOLDER,
                    "Root folder",
                    "The folder Radarr puts each Movie in.",
                    SettingKind::SETTING_KIND_CHOICE,
                ),
            ],
            ..nama_pdk::manifest("radarr", env!("CARGO_PKG_VERSION"))
        }))
    }

    // Offers Radarr's quality profiles, by id, and its root folders, by path.
    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        let radarr = Radarr::from_config()?;
        let profiles: Vec<QualityProfile> = radarr.get("qualityprofile")?;
        let folders: Vec<RootFolder> = radarr.get("rootfolder")?;
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

    /// Adds each Movie to Radarr, which starts a search for it. Its reference is the Movie's TMDB
    /// id, which Radarr also knows it by.
    struct RadarrFulfiller;

    impl Fulfiller for RadarrFulfiller {
        fn fulfill(request: Request) -> Result<String, Error> {
            let radarr = Radarr::from_config()?;
            let movie = NewMovie {
                tmdb_id: request.movie.tmdb_id,
                title: request.movie.title.clone(),
                quality_profile_id: setting(QUALITY_PROFILE)?
                    .parse()
                    .map_err(|err| Error::msg(format!("the quality profile isn't an id: {err}")))?,
                root_folder_path: setting(ROOT_FOLDER)?,
                monitored: true,
                minimum_availability: "released",
                add_options: AddOptions {
                    search_for_movie: true,
                },
            };
            let response = send(&radarr.request("POST", "movie"), Some(Json(movie)))?;
            if is_success(&response) || already_has(&response) {
                Ok(request.movie.tmdb_id.to_string())
            } else {
                Err(refused(&response))
            }
        }

        /// Reads Radarr's whole queue in one call, whatever the number of `references`. A Movie
        /// that isn't in it has no downloads, and Availability comes from the Library, not here
        /// (ADR 0012).
        fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            // ponytail: reads only the first 1000 entries; page through `totalRecords` if a
            // queue ever holds more.
            let queue: Queue =
                Radarr::from_config()?.get("queue?pageSize=1000&includeMovie=true")?;
            let in_batch = queue
                .records
                .into_iter()
                .filter_map(|entry| Some((entry.reference()?, entry)))
                .filter(|(reference, _)| references.contains(reference));
            let mut downloads: HashMap<String, Vec<Download>> = HashMap::new();
            for (reference, entry) in in_batch {
                downloads.entry(reference).or_default().push(entry.into());
            }
            Ok(downloads)
        }
    }

    nama_pdk::fulfiller!(RadarrFulfiller);

    /// Radarr's API, as the settings say where it is and its key.
    struct Radarr {
        url: String,
        api_key: String,
    }

    impl Radarr {
        fn from_config() -> Result<Self, Error> {
            Ok(Self {
                url: setting(URL)?,
                api_key: setting(API_KEY)?,
            })
        }

        /// A `method` request for the API path `path`, such as `movie`.
        fn request(&self, method: &str, path: &str) -> HttpRequest {
            let url = format!("{}/api/v3/{path}", self.url.trim_end_matches('/'));
            HttpRequest::new(url)
                .with_method(method)
                .with_header("X-Api-Key", &self.api_key)
                .with_header("Content-Type", "application/json")
        }

        /// What Radarr answers at the API path `path`.
        fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
            let response = send(&self.request("GET", path), None::<()>)?;
            if !is_success(&response) {
                return Err(refused(&response));
            }
            response
                .json()
                .map_err(|err| Error::msg(format!("Radarr's answer made no sense: {err}")))
        }
    }

    fn send<T: ToMemory>(request: &HttpRequest, body: Option<T>) -> Result<HttpResponse, Error> {
        // Not naming the URL: a Request's reason reaches every Requester.
        nama_pdk::http_request(request, body, "Radarr didn't answer")
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
            .any(|failure| failure.error_code.as_deref() == Some(MOVIE_EXISTS))
    }

    /// Why Radarr refused a request, in words a User understands.
    fn refused(response: &HttpResponse) -> Error {
        let messages: Vec<_> = failures(response)
            .into_iter()
            .map(|failure| failure.error_message)
            .collect();
        let reason = match response.status_code() {
            401 => "Radarr refused the API key".to_owned(),
            400 if !messages.is_empty() => format!("Radarr said: {}", messages.join(" ")),
            status => format!("Radarr failed with HTTP status {status}"),
        };
        Error::msg(reason)
    }

    /// The validation failures in the body of a `400 Bad Request` from Radarr, if it is one.
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

    /// One download in Radarr's queue. Radarr may send `null` for those that are `Option`s.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QueueEntry {
        /// Only there with `includeMovie=true`.
        movie: Option<QueuedMovie>,
        title: Option<String>,
        size: f64,
        sizeleft: f64,
        /// How long it has left, as a .NET `TimeSpan`.
        timeleft: Option<String>,
        /// Where the download client has it, such as `downloading` or `completed`.
        status: String,
        /// `ok`, `warning` or `error`.
        tracked_download_status: Option<String>,
        /// Where Radarr has it, such as `downloading`, `importBlocked` or `importing`.
        tracked_download_state: Option<String>,
        status_messages: Option<Vec<StatusMessage>>,
        error_message: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct QueuedMovie {
        tmdb_id: i64,
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
                // Radarr gives a queued download's time left as zero.
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
        /// The reference `fulfill` gave its Movie: its TMDB id.
        fn reference(&self) -> Option<String> {
            Some(self.movie.as_ref()?.tmdb_id.to_string())
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

        /// Whether it waits on a person, rather than on Radarr or its download client.
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

        /// Radarr's messages about it, if it has any.
        fn attention_reason(&self) -> Option<String> {
            let mut messages: Vec<&str> = self
                .status_messages
                .iter()
                .flatten()
                .flat_map(|status| status.messages.iter().flatten())
                .map(String::as_str)
                .collect();
            messages.extend(self.error_message.as_deref());
            (!messages.is_empty()).then(|| format!("Radarr said: {}", messages.join(" ")))
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
            reason = "Radarr counts bytes in a double; only a positive size is kept"
        )]
        fn size_bytes(&self) -> Option<u64> {
            (self.size > 0.0).then_some(self.size as u64)
        }
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

    /// What `POST /api/v3/movie` takes to add a Movie.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct NewMovie {
        tmdb_id: i64,
        title: String,
        quality_profile_id: i64,
        root_folder_path: String,
        monitored: bool,
        /// Not before it is released, so Radarr doesn't settle for a cinema recording.
        minimum_availability: &'static str,
        add_options: AddOptions,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AddOptions {
        search_for_movie: bool,
    }
}
