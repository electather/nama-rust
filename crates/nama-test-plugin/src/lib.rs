//! An Installed Plugin used only in the Server's tests. Build it with
//! `cargo build -p nama-test-plugin --target wasm32-unknown-unknown`.

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use std::{collections::HashMap, time::UNIX_EPOCH};

    use nama_pdk::{
        Buffa, CheckedSettings, Download, FnResult, Fulfilled, Fulfiller, Manifest, Notification,
        Notifier, Recipient, Request,
        extism_pdk::{self, Error, HttpRequest},
        nama_plugin_proto::nama::plugin::v1::{
            ObservedEvents, PersonalSettingsCheck, PersonalSettingsChecked, SettingField,
            SettingKind, SettingValue, request::MovieOrSeason, setting_value,
        },
        plugin_fn, services,
    };

    /// The setting that picks what its Observer does: `log`, `clock`, `hoard`, `loop` or
    /// `panic`.
    const OBSERVER: &str = "observer";

    /// The setting that picks what its Fulfiller does: `take`, `error`, `loop` or `panic`.
    const FULFILLER: &str = "fulfiller";

    /// The setting that picks what its Notifier does: `post`, `error`, `loop` or `panic`.
    const NOTIFIER: &str = "notifier";

    /// The Personal setting naming whom a User's notifications go to.
    pub(super) const HANDLE: &str = "handle";

    /// The setting that makes its settings check fail with its text, or hoard 256 MiB, loop
    /// forever or panic for `hoard`, `loop` or `panic`.
    const FAIL_WITH: &str = "fail_with";

    // Says who the Plugin is, that it observes Events, fulfills Movie Requests and notifies, and
    // its settings: a URL whose host it may reach, one it requests, how its Observer, Fulfiller
    // and Notifier behave, a profile its check offers, and how its check fails, its Personal
    // settings, a `handle` and a `token`, and its logo. With the `unexported-connector` feature it
    // names a Connector it doesn't export, with `contract-0-1` it wants Plugin contract 0.1, with
    // `empty-points` it names no Extension Points, with `automation` it runs a Job every minute
    // and requests the Store and Library Permissions, with `jobs` it runs a Job daily at 04:00 and
    // one with no schedule, with `recogniser` it recognises files, with
    // `home-rows` it gives Home rows and requests the Store and Read Watch States Permissions, with
    // `subtitle-source` it is a subtitle source that finds nothing, with `similar-source` a
    // Similar source answering what its `similar` setting says, with `watch-sync` a Watch sync
    // that logs what it is told, and with `panic` it panics instead.
    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        #[expect(
            clippy::panic,
            reason = "with the `panic` feature it panics on purpose"
        )]
        let manifest = if cfg!(feature = "panic") {
            panic!("the test Plugin was built to panic")
        } else {
            Manifest {
                name: "Test Plugin".to_owned(),
                description: "Stands in for an Installed Plugin in the Server's tests.".to_owned(),
                points: vec![
                    nama_pdk::point(services::MOVIE_FULFILLER),
                    nama_pdk::point(services::NOTIFIER),
                    nama_pdk::point(services::EVENT_OBSERVER),
                ],
                settings: vec![
                    setting("url", SettingKind::SETTING_KIND_URL),
                    setting("fetch", SettingKind::SETTING_KIND_TEXT),
                    behaviour_setting(
                        OBSERVER,
                        "log",
                        "when told about Events: log, clock, hoard, loop, panic or garble",
                    ),
                    behaviour_setting(
                        FULFILLER,
                        "take",
                        "when handed a Request: take, error, loop or panic",
                    ),
                    behaviour_setting(
                        NOTIFIER,
                        "post",
                        "when handed a notification: post, error, loop or panic",
                    ),
                    setting("profile", SettingKind::SETTING_KIND_CHOICE),
                    setting(FAIL_WITH, SettingKind::SETTING_KIND_TEXT),
                ],
                personal_settings: vec![
                    SettingField {
                        required: true,
                        ..setting(HANDLE, SettingKind::SETTING_KIND_TEXT)
                    },
                    setting("token", SettingKind::SETTING_KIND_SECRET),
                ],
                logo_svg: include_str!("../logo.svg").to_owned(),
                ..nama_pdk::manifest("test-plugin", env!("CARGO_PKG_VERSION"))
            }
        };
        #[cfg(feature = "unexported-connector")]
        let manifest = Manifest {
            points: vec![
                nama_pdk::point(services::CONNECTOR),
                nama_pdk::point(services::EVENT_OBSERVER),
            ],
            ..manifest
        };
        #[cfg(feature = "empty-points")]
        let manifest = Manifest {
            points: Vec::new(),
            ..manifest
        };
        #[cfg(feature = "automation")]
        let manifest = Manifest {
            points: [manifest.points, vec![nama_pdk::point(services::JOB)]].concat(),
            permissions: vec![
                nama_pdk::Permission::PERMISSION_STORE.into(),
                nama_pdk::Permission::PERMISSION_READ_LIBRARIES.into(),
            ],
            jobs: vec![job("kernel", Some("* * * * *"))],
            ..manifest
        };
        #[cfg(feature = "jobs")]
        let manifest = Manifest {
            points: [manifest.points, vec![nama_pdk::point(services::JOB)]].concat(),
            jobs: vec![job("wait", Some("0 4 * * *")), job("on-ask", None)],
            ..manifest
        };
        #[cfg(feature = "recogniser")]
        let manifest = Manifest {
            points: [manifest.points, vec![nama_pdk::point(services::RECOGNISER)]].concat(),
            ..manifest
        };
        #[cfg(feature = "home-rows")]
        let manifest = Manifest {
            points: [manifest.points, vec![nama_pdk::point(services::HOME_ROW)]].concat(),
            permissions: [
                manifest.permissions,
                vec![
                    nama_pdk::Permission::PERMISSION_STORE.into(),
                    nama_pdk::Permission::PERMISSION_READ_WATCH_STATES.into(),
                ],
            ]
            .concat(),
            settings: [
                manifest.settings,
                vec![setting(
                    crate::home_rows::ROWS,
                    SettingKind::SETTING_KIND_TEXT,
                )],
            ]
            .concat(),
            ..manifest
        };
        #[cfg(feature = "subtitle-source")]
        let manifest = Manifest {
            points: [
                manifest.points,
                vec![nama_pdk::point(services::SUBTITLE_SOURCE)],
            ]
            .concat(),
            ..manifest
        };
        #[cfg(feature = "similar-source")]
        let manifest = Manifest {
            points: [
                manifest.points,
                vec![nama_pdk::point(services::SIMILAR_SOURCE)],
            ]
            .concat(),
            settings: [
                manifest.settings,
                vec![setting(
                    crate::similar_source::SIMILAR,
                    SettingKind::SETTING_KIND_TEXT,
                )],
            ]
            .concat(),
            ..manifest
        };
        #[cfg(feature = "watch-sync")]
        let manifest = Manifest {
            points: [manifest.points, vec![nama_pdk::point(services::WATCH_SYNC)]].concat(),
            ..manifest
        };
        #[cfg(feature = "contract-0-1")]
        let manifest = Manifest {
            contract: "^0.1.0".to_owned(),
            ..manifest
        };
        Ok(Buffa(manifest))
    }

    fn setting(key: &str, kind: SettingKind) -> SettingField {
        SettingField {
            key: key.to_owned(),
            label: key.to_owned(),
            kind: kind.into(),
            ..SettingField::default()
        }
    }

    /// The text setting `key`, `default` unless set, that picks what the Plugin does `when`.
    /// Required, so a test sees a default fill a required setting.
    fn behaviour_setting(key: &str, default: &str, when: &str) -> SettingField {
        SettingField {
            description: format!("What it does {when}."),
            required: true,
            default_value: SettingValue {
                value: Some(setting_value::Value::TextValue(default.to_owned())),
                ..Default::default()
            }
            .into(),
            ..setting(key, SettingKind::SETTING_KIND_TEXT)
        }
    }

    // Logs each Event it is told about, so a test can see them arrive, then requests its `fetch`
    // setting's URL, if it has one, and logs the response's status and `Retry-After`. Its
    // `observer` setting can make it log the time by `nama_pdk::now` instead, or hoard 256 MiB,
    // loop forever or panic, so a test can see the Server survive that. Set to `garble`, it
    // answers with bytes that don't decode, so a test can see that logged as a failure (#1117).
    #[plugin_fn]
    pub fn observe_events(Buffa(observed): Buffa<ObservedEvents>) -> FnResult<Vec<u8>> {
        match nama_pdk::setting(OBSERVER)?.as_deref() {
            Some("clock") => {
                let since_epoch = nama_pdk::now().duration_since(UNIX_EPOCH)?;
                nama_pdk::info!("test-plugin's clock reads {}", since_epoch.as_millis());
            }
            Some("hoard") => {
                std::hint::black_box(vec![1_u8; 256 << 20]);
                nama_pdk::info!("test-plugin hoarded");
            }
            Some("loop") => loop_forever(),
            #[expect(clippy::panic, reason = "it panics on purpose")]
            Some("panic") => panic!("the test Plugin's Observer was set to panic"),
            // A field's tag with its value cut off, which no message decodes.
            Some("garble") => return Ok(vec![0x08]),
            _ => {
                for event in observed.events {
                    nama_pdk::info!("test-plugin observed {event:?}");
                }
                if let Some(url) = nama_pdk::setting("fetch")? {
                    let response =
                        ::extism_pdk::http::request::<()>(&HttpRequest::new(&url), None)?;
                    // Read through `nama_pdk::Response`, in another case than the lowercase name
                    // the Server passes on. Only wasm32, where this export runs, converts it.
                    #[cfg(target_arch = "wasm32")]
                    let response = nama_pdk::Response::from(response);
                    let retry_after = response.header("Retry-After").unwrap_or("none");
                    let status = response.status_code();
                    nama_pdk::info!(
                        "test-plugin fetched {url}: {status}, Retry-After {retry_after}"
                    );
                }
            }
        }
        // An empty `EventsObserved`.
        Ok(Vec::new())
    }

    /// Loops, logging each turn, so a test sees `nama.log` leave the call's clock running.
    pub(super) fn loop_forever() -> ! {
        nama_pdk::info!("test-plugin is looping");
        loop {
            nama_pdk::debug!("test-plugin is still looping");
        }
    }

    /// A Movie Fulfiller that, as its `fulfiller` setting says, takes each Request, giving
    /// `test-<TMDB id>` as its reference, refuses it, loops forever or panics. It says each
    /// reference it is asked about is half downloaded.
    struct TestFulfiller;

    impl Fulfiller for TestFulfiller {
        fn fulfill(request: Request) -> Result<Fulfilled, Error> {
            match nama_pdk::setting(FULFILLER)?.as_deref() {
                Some("error") => Err(Error::msg("the test Plugin's Fulfiller was set to error")),
                Some("loop") => loop_forever(),
                #[expect(clippy::panic, reason = "it panics on purpose")]
                Some("panic") => panic!("the test Plugin's Fulfiller was set to panic"),
                _ => match request.movie_or_season {
                    Some(MovieOrSeason::Movie(movie)) => Ok(Fulfilled {
                        reference: format!(
                            "test-{}",
                            nama_pdk::title_id(&movie.ids, "tmdb").unwrap_or_default()
                        ),
                        ..Fulfilled::default()
                    }),
                    _ => Err(Error::msg("the test Plugin takes only Movie Requests")),
                },
            }
        }

        fn progress(references: Vec<String>) -> Result<HashMap<String, Vec<Download>>, Error> {
            let half_done = |reference: String| {
                let download = Download {
                    title: reference.clone(),
                    percent: 50.0,
                    ..Download::default()
                };
                (reference, vec![download])
            };
            Ok(references.into_iter().map(half_done).collect())
        }
    }

    nama_pdk::fulfiller!(TestFulfiller);

    /// A Notifier that, as its `notifier` setting says, posts each notification for its channel
    /// or for a User, encoded as protobuf, to its `url` setting's URL if it has one, fails, loops
    /// forever or panics. A User's post names their `handle` in a `Recipient` header.
    struct TestNotifier;

    impl Notifier for TestNotifier {
        fn deliver(notification: Notification, recipient: Recipient) -> Result<(), Error> {
            let target = match recipient {
                Recipient::Channel(_) => None,
                Recipient::User(user) => Some(
                    user.personal_settings
                        .get(HANDLE)
                        .cloned()
                        .ok_or_else(|| Error::msg("the User set no handle"))?,
                ),
                Recipient::Connection(_) => return Ok(()),
            };
            match nama_pdk::setting(NOTIFIER)?.as_deref() {
                Some("error") => Err(Error::msg("the test Plugin's Notifier was set to error")),
                Some("loop") => loop_forever(),
                #[expect(clippy::panic, reason = "it panics on purpose")]
                Some("panic") => panic!("the test Plugin's Notifier was set to panic"),
                _ => post(notification, target.as_deref()),
            }
        }
    }

    nama_pdk::notifier!(TestNotifier);

    /// Posts `notification`, encoded as protobuf, to its `url` setting's URL, if it has one,
    /// naming `target` in a `Recipient` header if there is one.
    fn post(notification: Notification, target: Option<&str>) -> Result<(), Error> {
        if let Some(url) = nama_pdk::setting("url")? {
            let mut request = HttpRequest::new(&url).with_method("POST");
            if let Some(target) = target {
                request = request.with_header("Recipient", target);
            }
            ::extism_pdk::http::request(&request, Some(Buffa(notification)))?;
        }
        Ok(())
    }

    // Requests its `url` setting's URL, if it has one, as Sonarr's check would, then offers two
    // profiles. Its `fail_with` setting makes it fail, hoard 256 MiB, loop forever or panic
    // instead.
    #[plugin_fn]
    pub fn check_settings() -> FnResult<Buffa<CheckedSettings>> {
        match nama_pdk::setting(FAIL_WITH)?.as_deref() {
            Some("loop") => loop_forever(),
            #[expect(clippy::panic, reason = "it panics on purpose")]
            Some("panic") => panic!("the test Plugin's check was set to panic"),
            Some("hoard") => {
                std::hint::black_box(vec![1_u8; 256 << 20]);
                Err(Error::msg("test-plugin hoarded").into())
            }
            Some(error) => Err(Error::msg(error.to_owned()).into()),
            None => {
                if let Some(url) = nama_pdk::setting("url")? {
                    ::extism_pdk::http::request::<()>(&HttpRequest::new(&url), None)?;
                }
                let profiles = [("1", "HD"), ("2", "Ultra HD")];
                Ok(Buffa(nama_pdk::checked_settings([("profile", profiles)])))
            }
        }
    }

    // Refuses the `handle` `taken` as taken, and the `handle` `echo` quoting the `token`, so a
    // test sees a secret blanked. The `handle` `loop` loops forever and `panic` panics; any other
    // passes.
    #[plugin_fn]
    pub fn check_personal_settings(
        Buffa(asked): Buffa<PersonalSettingsCheck>,
    ) -> FnResult<Buffa<PersonalSettingsChecked>> {
        let value = |key: &str| asked.personal_settings.get(key).map(String::as_str);
        match value(HANDLE) {
            Some("taken") => Err(Error::msg("the handle taken is someone else's").into()),
            Some("echo") => {
                let token = value("token").unwrap_or_default();
                Err(Error::msg(format!("the token {token} doesn't open echo")).into())
            }
            Some("loop") => loop_forever(),
            #[expect(clippy::panic, reason = "it panics on purpose")]
            Some("panic") => panic!("the test Plugin's Personal settings check was set to panic"),
            _ => Ok(Buffa(PersonalSettingsChecked::default())),
        }
    }

    #[cfg(any(feature = "automation", feature = "jobs"))]
    fn job(id: &str, schedule: Option<&str>) -> nama_pdk::Job {
        nama_pdk::Job {
            id: id.to_owned(),
            name: id.to_owned(),
            schedule: schedule.map(str::to_owned),
            ..nama_pdk::Job::default()
        }
    }

    /// Runs its Jobs. `kernel` keeps a value in its Store, reads it back and lists the Libraries,
    /// failing with the Kernel's words when it refuses any of them; with the `looping-job` feature
    /// it loops forever instead. The others log when they start, with the Kernel's clock, and end,
    /// and `wait` spins until that clock reads a minute past its start, so a test brings its next
    /// firing due while it runs.
    #[cfg(any(feature = "automation", feature = "jobs"))]
    struct TestJobs;

    #[cfg(any(feature = "automation", feature = "jobs"))]
    impl nama_pdk::JobRunner for TestJobs {
        fn run(run: nama_pdk::RunJob) -> Result<(), Error> {
            let job = &run.job_id;
            if job == "kernel" {
                return use_the_kernel();
            }
            nama_pdk::info!("test-plugin's Job {job} started at {}", run.now_ms);
            if job == "wait" {
                let started = UNIX_EPOCH
                    + std::time::Duration::from_millis(run.now_ms.try_into().unwrap_or_default());
                let until = started + std::time::Duration::from_secs(60);
                while nama_pdk::now() < until {}
            }
            nama_pdk::info!("test-plugin's Job {job} ended");
            Ok(())
        }
    }

    #[cfg(any(feature = "automation", feature = "jobs"))]
    nama_pdk::job_runner!(TestJobs);

    /// The Job `kernel`: see [`TestJobs`].
    #[cfg(any(feature = "automation", feature = "jobs"))]
    fn use_the_kernel() -> Result<(), Error> {
        if cfg!(feature = "looping-job") {
            loop_forever();
        }
        nama_pdk::kernel::store_put("ran", None, b"yes")?;
        let kept = nama_pdk::kernel::store_get("ran", None)?;
        nama_pdk::info!("test-plugin's Store keeps {kept:?}");
        let libraries = nama_pdk::kernel::libraries()?;
        nama_pdk::info!("test-plugin sees {} Libraries", libraries.len());
        Ok(())
    }

    /// A Recogniser reading `<Show> - <n>.<ext>` as Episode n of the Show's Season 1, numbered
    /// from the start of the Show as anime often is. A `(<year>)` ending the Show's name is the
    /// year it first aired. It can't tell anything else.
    #[cfg(feature = "recogniser")]
    struct TestRecogniser;

    #[cfg(feature = "recogniser")]
    impl nama_pdk::Recogniser for TestRecogniser {
        fn recognise_file(
            file: nama_pdk::FileToRecognise,
        ) -> Result<nama_pdk::FileRecognised, Error> {
            let name = file.path.rsplit('/').next().unwrap_or_default();
            let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
            let Some((show, number)) = stem.rsplit_once(" - ") else {
                return Ok(nama_pdk::FileRecognised::default());
            };
            let Ok(episode) = number.trim().parse() else {
                return Ok(nama_pdk::FileRecognised::default());
            };
            let (show_title, show_year) = match show.trim_end().strip_suffix(')') {
                Some(rest) => match rest.rsplit_once(" (") {
                    Some((title, year)) => (title, year.parse().ok()),
                    None => (show, None),
                },
                None => (show, None),
            };
            let episodes = nama_pdk::RecognisedEpisodes {
                show_title: show_title.to_owned(),
                show_year,
                season: 1,
                episodes: vec![episode],
                ..nama_pdk::RecognisedEpisodes::default()
            };
            Ok(nama_pdk::FileRecognised {
                recognised: Some(episodes.into()),
                ..nama_pdk::FileRecognised::default()
            })
        }
    }

    #[cfg(feature = "recogniser")]
    nama_pdk::recogniser!(TestRecogniser, [recognise_file]);
}

#[cfg(feature = "home-rows")]
mod home_rows {
    use nama_pdk::{
        FeaturedSuggestion, FeaturedSuggestions, HomeRow, HomeRowSource, HomeRowTitle, HomeRows,
        TitleId, TitleKind, UserFeaturedAsked, UserRowsAsked, extism_pdk::Error,
    };

    /// The setting naming the keys of the rows it gives, comma-separated, all of them when
    /// empty, `loop` to loop until the Server interrupts it, or a value ending in `fail` to fail,
    /// saying the value, so two tests counting the log lines fail apart.
    pub(super) const ROWS: &str = "home_rows";

    /// Gives every User `picks`: the Movies Heat, Dune and Alien and the Show Dark, which the
    /// fake TMDB knows; `again`: Alien; and `missing`: Dune and a Movie no title source has. Gives
    /// a User who set a `handle` the row `for-<handle>`: Alien, and suggests for their Featured
    /// Heat at 1.0, Alien at 0.0 and a Movie no title source has at 1.0. Logs which it was asked
    /// for, so a test counts its refreshes, and what its Store kept or why the Kernel refused it,
    /// so a test sees a Plugin that isn't an Automation call the Kernel's functions. For the
    /// `handle` [`WAITS`] it answers a User's rows and Featured only once the clock has moved.
    struct TestRows;

    /// The `handle` whose rows and Featured are answered only once the Server's clock reads a
    /// minute past when they were asked, so a test can change something while a refresh runs.
    const WAITS: &str = "waits";

    /// Spins until a minute past now on the Server's clock, if `handle` is [`WAITS`].
    fn wait_a_clock_minute_for(handle: &str) {
        if handle == WAITS {
            let until = nama_pdk::now() + std::time::Duration::from_secs(60);
            while nama_pdk::now() < until {}
        }
    }

    impl HomeRowSource for TestRows {
        fn shared_rows() -> Result<HomeRows, Error> {
            let asked = nama_pdk::setting(ROWS)?.unwrap_or_default();
            nama_pdk::info!("test-plugin gives Home rows {asked:?}");
            match keep_and_read_back() {
                Ok(kept) => nama_pdk::info!("test-plugin's Store keeps {kept:?}"),
                // A Plugin carries on without the Store (ADR 0071).
                Err(refused) => nama_pdk::warn!("test-plugin's Store refused: {refused}"),
            }
            if asked == "loop" {
                super::exports::loop_forever();
            }
            if asked.ends_with("fail") {
                return Err(Error::msg(format!("test-plugin was set to {asked}")));
            }
            let rows = [
                row(
                    "picks",
                    "Picks",
                    &[(MOVIE, 949), (MOVIE, 438_631), (MOVIE, 348), (SHOW, 70_523)],
                ),
                row("again", "Alien again", &[(MOVIE, 348)]),
                row(
                    "missing",
                    "Not on the Server",
                    &[(MOVIE, 438_631), (MOVIE, 900_001)],
                ),
            ];
            Ok(HomeRows {
                rows: (rows.into_iter())
                    .filter(|row| asked.is_empty() || asked.split(',').any(|key| key == row.key))
                    .collect(),
                ..HomeRows::default()
            })
        }

        fn user_rows(asked: UserRowsAsked) -> Result<HomeRows, Error> {
            let Some(handle) = asked.personal_settings.get(super::exports::HANDLE) else {
                return Ok(HomeRows::default());
            };
            nama_pdk::info!("test-plugin gives {handle}'s Home rows");
            wait_a_clock_minute_for(handle);
            Ok(HomeRows {
                rows: vec![row(
                    &format!("for-{handle}"),
                    &format!("For {handle}"),
                    &[(MOVIE, 348)],
                )],
                ..HomeRows::default()
            })
        }

        fn user_featured(asked: UserFeaturedAsked) -> Result<FeaturedSuggestions, Error> {
            let Some(handle) = asked.personal_settings.get(super::exports::HANDLE) else {
                return Ok(FeaturedSuggestions::default());
            };
            nama_pdk::info!("test-plugin suggests {handle}'s Featured");
            wait_a_clock_minute_for(handle);
            let suggestion = |(tmdb, weight): (i64, f32)| FeaturedSuggestion {
                kind: MOVIE.into(),
                id: tmdb_id(tmdb).into(),
                weight,
                ..FeaturedSuggestion::default()
            };
            Ok(FeaturedSuggestions {
                suggestions: [(949, 1.0), (348, 0.0), (900_001, 1.0)]
                    .map(suggestion)
                    .into(),
                ttl_seconds: 60 * 60,
                ..FeaturedSuggestions::default()
            })
        }
    }

    /// Keeps the text "rows" in the Plugin's Store and reads back what the Store then keeps.
    fn keep_and_read_back() -> Result<Option<String>, Error> {
        nama_pdk::kernel::store_put("asked", None, b"rows")?;
        let kept = nama_pdk::kernel::store_get("asked", None)?;
        Ok(kept.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    }

    const MOVIE: TitleKind = TitleKind::TITLE_KIND_MOVIE;
    const SHOW: TitleKind = TitleKind::TITLE_KIND_SHOW;

    /// The row `key`, headed `heading`, of the titles of `titles`' kinds and TMDB ids, fresh for
    /// an hour.
    fn row(key: &str, heading: &str, titles: &[(TitleKind, i64)]) -> HomeRow {
        let title = |&(kind, tmdb): &(TitleKind, i64)| HomeRowTitle {
            kind: kind.into(),
            id: tmdb_id(tmdb).into(),
            ..HomeRowTitle::default()
        };
        HomeRow {
            key: key.to_owned(),
            heading: heading.to_owned(),
            ttl_seconds: 60 * 60,
            titles: titles.iter().map(title).collect(),
            ..HomeRow::default()
        }
    }

    fn tmdb_id(tmdb: i64) -> TitleId {
        TitleId {
            namespace: "tmdb".to_owned(),
            value: tmdb.to_string(),
            ..TitleId::default()
        }
    }

    #[cfg(not(any(feature = "home-rows-shared-only", feature = "home-rows-user-only")))]
    nama_pdk::home_row_source!(TestRows, [shared_rows, user_rows, user_featured]);
    #[cfg(feature = "home-rows-shared-only")]
    nama_pdk::home_row_source!(TestRows, [shared_rows]);
    #[cfg(feature = "home-rows-user-only")]
    nama_pdk::home_row_source!(TestRows, [user_rows]);
}

#[cfg(feature = "subtitle-source")]
mod subtitle_source {
    use nama_pdk::{
        SubtitleDownload, SubtitleFile, SubtitleResults, SubtitleSearch, SubtitleSource,
        extism_pdk::Error,
    };

    /// A subtitle source that finds nothing and fails every download.
    struct NoSubtitles;

    impl SubtitleSource for NoSubtitles {
        fn search(_search: SubtitleSearch) -> Result<SubtitleResults, Error> {
            Ok(SubtitleResults::default())
        }

        fn download(_download: SubtitleDownload) -> Result<SubtitleFile, Error> {
            Err(Error::msg("test-plugin has no subtitles"))
        }
    }

    nama_pdk::subtitle_source!(NoSubtitles);
}

#[cfg(feature = "similar-source")]
mod similar_source {
    use nama_pdk::{
        SimilarAsked, SimilarSource, SimilarTitle, SimilarTitles, TitleId, extism_pdk::Error,
    };

    /// The setting saying what it answers about every title: `<namespace>-<value>@<weight>`,
    /// comma-separated, none when empty, or `fail` to fail and `loop` to loop until the Server
    /// interrupts it.
    pub(super) const SIMILAR: &str = "similar";

    /// A Similar source answering the same titles, those its [`SIMILAR`] setting names, whatever
    /// it is asked about.
    struct Configured;

    impl SimilarSource for Configured {
        fn similar(_asked: SimilarAsked) -> Result<SimilarTitles, Error> {
            let answer = nama_pdk::setting(SIMILAR)?.unwrap_or_default();
            match answer.as_str() {
                "fail" => return Err(Error::msg("test-plugin was set to fail")),
                "loop" => super::exports::loop_forever(),
                _ => {}
            }
            let title = |named: &str| -> Option<SimilarTitle> {
                let (id, weight) = named.split_once('@')?;
                let (namespace, value) = id.split_once('-')?;
                Some(SimilarTitle {
                    id: TitleId {
                        namespace: namespace.to_owned(),
                        value: value.to_owned(),
                        ..TitleId::default()
                    }
                    .into(),
                    weight: weight.parse().ok()?,
                    ..SimilarTitle::default()
                })
            };
            Ok(SimilarTitles {
                titles: answer.split(',').filter_map(title).collect(),
                ..SimilarTitles::default()
            })
        }
    }

    nama_pdk::similar_source!(Configured);
}

#[cfg(feature = "watch-sync")]
mod watch_sync {
    use nama_pdk::{
        ImportAsked, Imported, LinkPolled, LinkStarted, PollLinkAsked, ScrobbleAsked, Scrobbled,
        StartLinkAsked, SyncedWatchState, WatchSync, extism_pdk::Error,
        nama_plugin_proto::nama::plugin::v1::TitleRefKind,
    };

    /// A Watch sync that logs each change it is told, with the User's `handle`, imports nothing,
    /// and links an account as soon as it is polled, its `token` naming the device code.
    struct LoggedScrobbles;

    impl WatchSync for LoggedScrobbles {
        fn scrobble(asked: ScrobbleAsked) -> Result<Scrobbled, Error> {
            let handle = asked.personal_settings.get(super::exports::HANDLE);
            let handle = handle.map_or("nobody", String::as_str);
            for change in &asked.changes {
                log_scrobbled(handle, change);
            }
            Ok(Scrobbled::default())
        }

        fn import(asked: ImportAsked) -> Result<Imported, Error> {
            Ok(Imported {
                cursor: asked.since,
                ..Imported::default()
            })
        }

        fn start_link(_asked: StartLinkAsked) -> Result<LinkStarted, Error> {
            Ok(LinkStarted {
                user_code: "TEST-CODE".to_owned(),
                verification_url: "https://tracker.example/activate".to_owned(),
                expires_in_seconds: 600,
                interval_seconds: 5,
                device_code: "test-device".to_owned(),
                ..LinkStarted::default()
            })
        }

        fn poll_link(asked: PollLinkAsked) -> Result<LinkPolled, Error> {
            let token = format!("token-of-{}", asked.device_code);
            Ok(LinkPolled {
                personal_settings: [("token".to_owned(), token)].into_iter().collect(),
                ..LinkPolled::default()
            })
        }
    }

    /// Logs `change` as scrobbled for the User with the `handle` `handle`.
    fn log_scrobbled(handle: &str, change: &SyncedWatchState) {
        let kind = if change.kind == TitleRefKind::TITLE_REF_KIND_EPISODE {
            "Episode"
        } else {
            "Movie"
        };
        let ids: Vec<String> = (change.ids.iter())
            .map(|id| format!("{}-{}", id.namespace, id.value))
            .collect();
        let played = if change.played { "played" } else { "unplayed" };
        nama_pdk::info!(
            "test-plugin scrobbled {handle}'s {kind} {} {played}",
            ids.join(",")
        );
    }

    nama_pdk::watch_sync!(LoggedScrobbles);
}
