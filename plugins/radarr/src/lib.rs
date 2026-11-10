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
        Buffa, CheckedSettings, Download, FnResult, Fulfilled, Fulfiller, Manifest, Request,
        extism_pdk::{self, Error, config},
        nama_proto::nama::plugin::v1::{ProvidePoint, SettingField, SettingKind},
        plugin_fn,
    };
    use nama_servarr::{Queued, Servarr};
    use serde::{Deserialize, Serialize};

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
            name: "Radarr".to_owned(),
            description: "Sends Approved Movie Requests to Radarr to download.".to_owned(),
            ..nama_pdk::manifest("radarr", env!("CARGO_PKG_VERSION"))
        }))
    }

    // Offers Radarr's quality profiles, by id, and its root folders, by path.
    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        let radarr = radarr()?;
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
    /// id, which Radarr also knows it by. A Movie Radarr already has keeps its own profile.
    struct RadarrFulfiller;

    impl Fulfiller for RadarrFulfiller {
        fn fulfill(request: Request) -> Result<Fulfilled, Error> {
            let radarr = radarr()?;
            let tmdb_id = request.movie.tmdb_id;
            let asked = quality_profile(&request)?;
            // Checked first, as Radarr may add a Movie with a profile it no longer has.
            let profiles: Vec<QualityProfile> = radarr.get("qualityprofile")?;
            let used_id = if profiles.iter().any(|profile| profile.id == asked) {
                add_movie(&radarr, &request, asked)?
            } else {
                // A Movie Radarr already has keeps its own profile, so the asked one needn't exist.
                let kept = profile_of(&radarr, tmdb_id)?;
                Some(kept.ok_or_else(|| no_such_profile(&request, asked))?)
            };
            let used = profiles
                .into_iter()
                .find(|profile| Some(profile.id) == used_id);
            Ok(Fulfilled {
                reference: tmdb_id.to_string(),
                profile_id: used_id.map(|id| id.to_string()).unwrap_or_default(),
                profile_name: used.map(|used| used.name).unwrap_or_default(),
                ..Fulfilled::default()
            })
        }

        /// Reads Radarr's whole queue in one call, whatever the number of `references`. A Movie
        /// that isn't in it has no downloads, and Availability comes from the Library, not here
        /// (ADR 0012).
        fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            radarr()?.progress::<QueuedMovie>(&references)
        }
    }

    nama_pdk::fulfiller!(RadarrFulfiller);

    /// Radarr's API, as the settings say where it is and its key.
    fn radarr() -> Result<Servarr, Error> {
        Ok(Servarr::new("Radarr", setting(URL)?, setting(API_KEY)?))
    }

    /// Adds the Movie `request` asks for at the quality profile `profile`, starting a search, and
    /// returns the id of the profile Radarr has it at: its own if it already had it.
    fn add_movie(radarr: &Servarr, request: &Request, profile: i64) -> Result<Option<i64>, Error> {
        let movie = NewMovie {
            tmdb_id: request.movie.tmdb_id,
            title: request.movie.title.clone(),
            quality_profile_id: profile,
            root_folder_path: setting(ROOT_FOLDER)?,
            monitored: true,
            minimum_availability: "released",
            add_options: AddOptions {
                search_for_movie: true,
            },
        };
        if radarr.add("movie", movie, MOVIE_EXISTS)? {
            Ok(Some(profile))
        } else {
            profile_of(radarr, request.movie.tmdb_id)
        }
    }

    /// The id of the quality profile Radarr has the Movie `tmdb_id` at, if it has the Movie.
    fn profile_of(radarr: &Servarr, tmdb_id: i64) -> Result<Option<i64>, Error> {
        let movies: Vec<AddedMovie> = radarr.get(&format!("movie?tmdbId={tmdb_id}"))?;
        Ok(movies.first().map(|movie| movie.quality_profile_id))
    }

    /// The Radarr profile id `request` is to be added with: its Quality's, else the setting's.
    fn quality_profile(request: &Request) -> Result<i64, Error> {
        let profile = match request.quality.as_option() {
            Some(quality) => quality.profile_id.clone(),
            None => setting(QUALITY_PROFILE)?,
        };
        profile
            .parse()
            .map_err(|err| Error::msg(format!("the quality profile isn't an id: {err}")))
    }

    /// Why `request` can't go to Radarr, which has no quality profile `id`.
    fn no_such_profile(request: &Request, id: i64) -> Error {
        let reason = format!("Radarr has no quality profile with the id {id}");
        Error::msg(match request.quality.as_option() {
            Some(quality) => format!("{reason}, which {} uses", quality.name),
            None => reason,
        })
    }

    /// The setting `key`, which the Server sets before every call but a settings check, and for
    /// that one too unless it is a Choice.
    fn setting(key: &str) -> Result<String, Error> {
        config::get(key)?.ok_or_else(|| Error::msg(format!("the setting {key} isn't set")))
    }

    /// What an entry in Radarr's queue is for.
    #[derive(Deserialize)]
    struct QueuedMovie {
        movie: Option<MovieIds>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MovieIds {
        tmdb_id: i64,
    }

    impl Queued for QueuedMovie {
        const INCLUDE: &'static str = "includeMovie";

        /// The reference `fulfill` gave its Movie: its TMDB id.
        fn reference(&self) -> Option<String> {
            Some(self.movie.as_ref()?.tmdb_id.to_string())
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

    /// A Movie Radarr has, as `GET /api/v3/movie` gives it.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AddedMovie {
        quality_profile_id: i64,
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
