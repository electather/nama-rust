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
        extism_pdk::{self, Error},
        nama_proto::nama::plugin::v1::ProvidePoint,
        plugin_fn,
    };
    use nama_servarr::{Queued, Servarr};
    use serde::{Deserialize, Serialize};

    /// The code Radarr gives when it refuses to add a Movie because it already has it.
    const MOVIE_EXISTS: &str = "MovieExistsValidator";

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(Manifest {
            logo_svg: include_str!("../logo.svg").to_owned(),
            providers: vec![ProvidePoint::PROVIDE_POINT_MOVIE_FULFILLER.into()],
            settings: nama_servarr::settings("Radarr", 7878),
            name: "Radarr".to_owned(),
            description: "Sends Approved Movie Requests to Radarr to download.".to_owned(),
            ..nama_pdk::manifest("radarr", env!("CARGO_PKG_VERSION"))
        }))
    }

    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        Ok(Buffa(radarr()?.checked_settings()?))
    }

    /// Adds each Movie to Radarr, which starts a search for it. Its reference is the Movie's TMDB
    /// id, which Radarr also knows it by. A Movie Radarr already has keeps its own profile.
    struct RadarrFulfiller;

    impl Fulfiller for RadarrFulfiller {
        fn fulfill(request: Request) -> Result<Fulfilled, Error> {
            let radarr = radarr()?;
            let tmdb_id = request.movie.tmdb_id;
            let asked = nama_servarr::quality_profile(&request)?;
            // Checked first, as Radarr may add a Movie with a profile it no longer has.
            let profiles = radarr.quality_profiles()?;
            let used_id = if profiles.iter().any(|profile| profile.id == asked) {
                add_movie(&radarr, &request, asked)?
            } else {
                // A Movie Radarr already has keeps its own profile, so the asked one needn't exist.
                let kept = profile_of(&radarr, tmdb_id)?;
                Some(kept.ok_or_else(|| radarr.no_such_profile(&request, asked))?)
            };
            Ok(nama_servarr::fulfilled(
                tmdb_id.to_string(),
                used_id,
                profiles,
            ))
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
        Servarr::from_settings("Radarr")
    }

    /// Adds the Movie `request` asks for at the quality profile `profile`, starting a search, and
    /// returns the id of the profile Radarr has it at: its own if it already had it.
    fn add_movie(radarr: &Servarr, request: &Request, profile: i64) -> Result<Option<i64>, Error> {
        let movie = NewMovie {
            tmdb_id: request.movie.tmdb_id,
            title: request.movie.title.clone(),
            quality_profile_id: profile,
            root_folder_path: nama_servarr::root_folder()?,
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
