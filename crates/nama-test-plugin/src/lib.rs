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
    const HANDLE: &str = "handle";

    /// The setting that makes its settings check fail with its text, or hoard 256 MiB, loop
    /// forever or panic for `hoard`, `loop` or `panic`.
    const FAIL_WITH: &str = "fail_with";

    // Says who the Plugin is, that it observes Events, fulfills Movie Requests and notifies, and
    // its settings: a URL whose host it may reach, one it requests, how its Observer, Fulfiller
    // and Notifier behave, a profile its check offers, and how its check fails, its Personal
    // settings, a `handle` and a `token`, and its logo. With the `unexported-connector` feature it
    // names a Connector it doesn't export, with `contract-0-1` it wants Plugin contract 0.1, with
    // `empty-points` it names no Extension Points, with `automation` it ticks every minute and
    // requests the Store and Library Permissions, with `recogniser` it recognises files, with
    // `home-rows` it gives Home rows, with `subtitle-source` it is a subtitle source that finds
    // nothing, and with `panic` it panics instead.
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
                        "when told about Events: log, clock, hoard, loop or panic",
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
            points: [manifest.points, vec![nama_pdk::point(services::AUTOMATION)]].concat(),
            permissions: vec![
                nama_pdk::Permission::PERMISSION_STORE.into(),
                nama_pdk::Permission::PERMISSION_READ_LIBRARIES.into(),
            ],
            tick_every_seconds: 60,
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
    // setting's URL, if it has one, and logs the response's status. Its `observer` setting can
    // make it log the time by `nama_pdk::now` instead, or hoard 256 MiB, loop forever or panic, so
    // a test can see the Server survive that.
    #[plugin_fn]
    pub fn observe_events(Buffa(observed): Buffa<ObservedEvents>) -> FnResult<()> {
        match nama_pdk::setting(OBSERVER)?.as_deref() {
            Some("clock") => {
                let since_epoch = nama_pdk::now().duration_since(UNIX_EPOCH)?;
                nama_pdk::info!("test-plugin's clock reads {}", since_epoch.as_millis());
                Ok(())
            }
            Some("hoard") => {
                std::hint::black_box(vec![1_u8; 256 << 20]);
                nama_pdk::info!("test-plugin hoarded");
                Ok(())
            }
            Some("loop") => loop_forever(),
            #[expect(clippy::panic, reason = "it panics on purpose")]
            Some("panic") => panic!("the test Plugin's Observer was set to panic"),
            _ => {
                for event in observed.events {
                    nama_pdk::info!("test-plugin observed {event:?}");
                }
                if let Some(url) = nama_pdk::setting("fetch")? {
                    let response =
                        ::extism_pdk::http::request::<()>(&HttpRequest::new(&url), None)?;
                    nama_pdk::info!("test-plugin fetched {url}: {}", response.status_code());
                }
                Ok(())
            }
        }
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

    /// An Automation whose tick keeps a value in its Store, reads it back and lists the
    /// Libraries, failing with the Kernel's words when it refuses any of them. With the
    /// `looping-tick` feature its tick loops forever instead.
    #[cfg(feature = "automation")]
    struct TestAutomation;

    #[cfg(feature = "automation")]
    impl nama_pdk::Automation for TestAutomation {
        fn tick(_tick: nama_pdk::Tick) -> Result<(), Error> {
            if cfg!(feature = "looping-tick") {
                loop_forever();
            }
            nama_pdk::kernel::store_put("ticked", None, b"yes")?;
            let kept = nama_pdk::kernel::store_get("ticked", None)?;
            nama_pdk::info!("test-plugin's Store keeps {kept:?}");
            let libraries = nama_pdk::kernel::libraries()?;
            nama_pdk::info!("test-plugin sees {} Libraries", libraries.len());
            Ok(())
        }
    }

    #[cfg(feature = "automation")]
    nama_pdk::automation!(TestAutomation);

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
        HomeRow, HomeRowSource, HomeRowTitle, HomeRows, TitleId, TitleKind, extism_pdk::Error,
    };

    /// The setting naming the keys of the rows it gives, comma-separated, all of them when
    /// empty, `loop` to loop until the Server interrupts it, or `fail` to fail.
    pub(super) const ROWS: &str = "home_rows";

    /// Gives `picks`: the Movies Heat, Dune and Alien and the Show Dark, which the fake TMDB
    /// knows; `again`: Alien; and `missing`: Dune and a Movie no title source has. Logs which it
    /// was asked for, so a test counts its refreshes.
    struct TestRows;

    impl HomeRowSource for TestRows {
        fn shared_rows() -> Result<HomeRows, Error> {
            let asked = nama_pdk::setting(ROWS)?.unwrap_or_default();
            nama_pdk::info!("test-plugin gives Home rows {asked:?}");
            if asked == "loop" {
                super::exports::loop_forever();
            }
            if asked == "fail" {
                return Err(Error::msg("test-plugin was set to fail"));
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
    }

    const MOVIE: TitleKind = TitleKind::TITLE_KIND_MOVIE;
    const SHOW: TitleKind = TitleKind::TITLE_KIND_SHOW;

    /// The row `key`, headed `heading`, of the titles of `titles`' kinds and TMDB ids, fresh for
    /// an hour.
    fn row(key: &str, heading: &str, titles: &[(TitleKind, i64)]) -> HomeRow {
        let title = |&(kind, tmdb): &(TitleKind, i64)| HomeRowTitle {
            kind: kind.into(),
            id: TitleId {
                namespace: "tmdb".to_owned(),
                value: tmdb.to_string(),
                ..TitleId::default()
            }
            .into(),
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

    nama_pdk::home_row_source!(TestRows);
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
