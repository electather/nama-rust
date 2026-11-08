//! The Radarr Installed Plugin: hands Approved Movie Requests to Radarr (ADR 0003). The Server
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

        // ponytail: says nothing is downloading until #146 reads Radarr's queue.
        fn progress(_references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            Ok(HashMap::new())
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
