use nama_pdk::{
    HomeRowSource, HomeRows, TitleKind,
    extism_pdk::Error,
    nama_plugin_proto::nama::plugin::v1::SettingKind,
    services,
    testing::{Kernel, with},
};
use url::Url;

use super::{
    API_KEY, LISTS, MdbList,
    fake_mdblist::{FakeList, FakeMdbList, VALID_KEY},
    manifest,
};

/// A Kernel holding the fake's API key and `lists` as List 1 to 3, whose requests `fake` answers.
fn kernel(fake: FakeMdbList, lists: &[&str]) -> Kernel {
    let kernel = Kernel::new()
        .setting(API_KEY, VALID_KEY)
        .http(move |request, _body| Ok(fake.answer(request)));
    (LISTS.into_iter().zip(lists)).fold(kernel, |kernel, (key, list)| kernel.setting(key, list))
}

/// The rows the Plugin answers with `lists` set, asking `fake`.
fn rows_from(fake: FakeMdbList, lists: &[&str]) -> Result<HomeRows, Error> {
    shared_rows(&mut kernel(fake, lists))
}

fn shared_rows(kernel: &mut Kernel) -> Result<HomeRows, Error> {
    with(kernel, MdbList::shared_rows)
}

/// The query of each request `kernel` sent for a list's items, without the API key.
fn pages(kernel: &Kernel) -> Vec<String> {
    (kernel.sent().iter())
        .filter_map(|sent| Url::parse(&sent.request.url).ok())
        .filter(|url| url.path().ends_with("/items"))
        .map(|url| {
            url.query()
                .unwrap_or_default()
                .replace(&format!("apikey={VALID_KEY}&"), "")
        })
        .collect()
}

/// A row's titles by kind and TMDB id.
type Titles = Vec<(Option<TitleKind>, String)>;

/// Each row's key, heading, time to live, and titles.
fn summary(rows: &HomeRows) -> Vec<(&str, &str, u32, Titles)> {
    rows.rows
        .iter()
        .map(|row| {
            let titles = row
                .titles
                .iter()
                .map(|title| {
                    let id = title.id.as_option().expect("each title has an id");
                    assert_eq!(id.namespace, "tmdb");
                    (title.kind.as_known(), id.value.clone())
                })
                .collect();
            (
                row.key.as_str(),
                row.heading.as_str(),
                row.ttl_seconds,
                titles,
            )
        })
        .collect()
}

#[test]
fn manifest_serves_home_rows_from_api_key_and_up_to_three_lists() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::HOME_ROW]);
    assert_eq!(manifest.allowed_hosts, ["api.mdblist.com"]);
    let settings: Vec<_> = manifest
        .settings
        .iter()
        .map(|field| (field.key.as_str(), field.kind.as_known(), field.required))
        .collect();
    let secret = Some(SettingKind::SETTING_KIND_SECRET);
    let text = Some(SettingKind::SETTING_KIND_TEXT);
    assert_eq!(
        settings,
        [
            ("api_key", secret, true),
            ("list_1", text, true),
            ("list_2", text, false),
            ("list_3", text, false),
        ]
    );
}

#[test]
fn manifest_says_where_to_get_an_api_key_and_gives_its_links_and_licence() {
    let manifest = manifest();

    let setup = &manifest.setup_markdown;
    assert!(
        setup.contains("https://mdblist.com/preferences/"),
        "{setup}"
    );
    assert_eq!(
        [manifest.homepage_url, manifest.source_url, manifest.license],
        [
            "https://mdblist.com/",
            "https://github.com/electather/nama-rust/tree/main/plugins/mdblist",
            "AGPL-3.0-only",
        ]
    );
}

#[test]
fn a_list_becomes_a_row_headed_by_its_name_of_its_tmdb_ids_in_rank_order() {
    let mut fake = FakeMdbList::default();
    fake.lists.push(FakeList {
        items: vec![("movie", 603), ("show", 1396), ("movie", 348)],
        ..FakeList::new(2194, "classics", "Classics")
    });

    let rows = rows_from(fake, &["https://mdblist.com/lists/alice/classics/"]).unwrap();

    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    let show = Some(TitleKind::TITLE_KIND_SHOW);
    assert_eq!(
        summary(&rows),
        [(
            "2194",
            "Classics",
            6 * 60 * 60,
            vec![
                (movie, "603".to_owned()),
                (show, "1396".to_owned()),
                (movie, "348".to_owned())
            ]
        )]
    );
}

#[test]
fn a_long_list_is_read_a_page_at_a_time_up_to_200_titles() {
    let mut fake = FakeMdbList::default();
    fake.lists.push(FakeList {
        items: (1..=250).map(|id| ("movie", id)).collect(),
        ..FakeList::new(7, "long", "Long")
    });

    let mut kernel = kernel(fake, &["7"]);

    let rows = shared_rows(&mut kernel).unwrap();

    let ids: Vec<_> = summary(&rows)[0]
        .3
        .iter()
        .map(|(_, id)| id.clone())
        .collect();
    let expected: Vec<_> = (1..=200).map(|id: u32| id.to_string()).collect();
    assert_eq!(ids, expected);
    assert_eq!(
        pages(&kernel),
        ["limit=100&offset=0", "limit=100&offset=100"]
    );
}

#[test]
fn a_short_list_stops_at_its_first_short_page() {
    let mut fake = FakeMdbList::default();
    fake.lists.push(FakeList {
        items: vec![("show", 1399)],
        ..FakeList::new(8, "short", "Short")
    });

    let mut kernel = kernel(fake, &["alice/short"]);

    shared_rows(&mut kernel).unwrap();

    assert_eq!(pages(&kernel), ["limit=100&offset=0"]);
}

#[test]
fn each_list_set_becomes_a_row_in_the_settings_order() {
    let mut fake = FakeMdbList::default();
    fake.lists.push(FakeList::new(1, "first", "First"));
    fake.lists.push(FakeList::new(2, "second", "Second"));

    let rows = rows_from(fake, &["2", "", "alice/first"]).unwrap();

    let headings: Vec<_> = rows.rows.iter().map(|row| row.heading.as_str()).collect();
    assert_eq!(headings, ["Second", "First"]);
}

#[test]
fn a_list_that_is_gone_fails_the_refresh_naming_it() {
    let mut fake = FakeMdbList::default();
    fake.lists.push(FakeList::new(1, "first", "First"));

    let err = rows_from(fake, &["alice/first", "alice/deleted"]).unwrap_err();

    assert_eq!(
        err.to_string(),
        "the MDBList list alice/deleted is gone or private"
    );
}

#[test]
fn a_private_list_fails_the_refresh_naming_it() {
    let mut fake = FakeMdbList::default();
    fake.lists.push(FakeList {
        private: true,
        ..FakeList::new(1, "secret", "Secret")
    });

    let err = rows_from(fake, &["https://mdblist.com/lists/alice/secret"]).unwrap_err();

    assert_eq!(
        err.to_string(),
        "the MDBList list https://mdblist.com/lists/alice/secret is gone or private"
    );
}

#[test]
fn a_setting_that_names_no_list_fails_the_refresh_saying_so() {
    let mut kernel = kernel(FakeMdbList::default(), &["alice/top/movies"]);

    let err = shared_rows(&mut kernel).unwrap_err();

    assert_eq!(
        err.to_string(),
        "alice/top/movies isn't an MDBList list's address or id"
    );
    assert_eq!(kernel.sent(), []);
}

#[test]
fn a_refused_api_key_fails_the_refresh_saying_so() {
    let mut fake = FakeMdbList::default();
    fake.lists.push(FakeList::new(1, "first", "First"));
    let mut kernel = kernel(fake, &["1"]).setting(API_KEY, "wrong");

    let err = shared_rows(&mut kernel).unwrap_err();

    assert_eq!(err.to_string(), "MDBList refused the API key");
}
