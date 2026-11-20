//! The MDBList Installed Plugin: a Home row source adding up to 3 public MDBList lists as rows
//! every User sees. The Server builds it for wasm32 and bundles it in its binary.

use std::time::Duration;

use nama_pdk::{
    HomeRow, HomeRowSource, HomeRowTitle, HomeRows, Manifest, TitleId, TitleKind,
    extism_pdk::{Error, HttpRequest, json},
    nama_plugin_proto::nama::plugin::v1::{SettingField, SettingKind},
    services,
};
use serde::Deserialize;
use url::Url;

const API_KEY: &str = "api_key";

/// The settings each naming one list, in the order home shows their rows.
const LISTS: [&str; 3] = ["list_1", "list_2", "list_3"];

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let list = |(number, key): (usize, &str)| SettingField {
        key: key.to_owned(),
        label: format!("List {}", number + 1),
        description: "A public MDBList list's address, such as \
                      https://mdblist.com/lists/garycrawfordgc/latest-tv-shows, or its id."
            .to_owned(),
        kind: SettingKind::SETTING_KIND_TEXT.into(),
        required: number == 0,
        ..SettingField::default()
    };
    let api_key = SettingField {
        key: API_KEY.to_owned(),
        label: "API key".to_owned(),
        description: "Your MDBList API key, from the Preferences page of your MDBList account."
            .to_owned(),
        kind: SettingKind::SETTING_KIND_SECRET.into(),
        required: true,
        ..SettingField::default()
    };
    Manifest {
        name: "MDBList".to_owned(),
        description: "Shows public MDBList lists as rows on the home page.".to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::HOME_ROW)],
        allowed_hosts: vec!["api.mdblist.com".to_owned()],
        settings: std::iter::once(api_key)
            .chain(LISTS.into_iter().enumerate().map(list))
            .collect(),
        ..nama_pdk::manifest("mdblist", env!("CARGO_PKG_VERSION"))
    }
}

/// Where MDBList's lists are, by id or by owner and name.
const LISTS_URL: &str = "https://api.mdblist.com/lists/";

/// The most titles a row holds: the Kernel keeps no more.
const MOST_TITLES: usize = 200;

/// The most items asked for in one request.
const PAGE_ITEMS: usize = 100;

/// How long a row stays fresh: a few hours keeps a row near its list without asking MDBList on
/// every visit to home.
const FRESH_FOR: Duration = Duration::from_hours(6);

/// Serves the lists the settings name as Home rows.
struct MdbList;

impl HomeRowSource for MdbList {
    fn shared_rows() -> Result<HomeRows, Error> {
        rows(nama_pdk::setting, send)
    }
}

nama_pdk::home_row_source!(MdbList);

/// MDBList's answer to a request: the Server gives a Plugin no response headers.
#[derive(Debug)]
struct Answer {
    status: u16,
    body: Vec<u8>,
}

/// GETs `url` through the Server, the only transport outside the tests.
fn send(url: &Url) -> Result<Answer, Error> {
    let response = nama_pdk::http_request(
        &HttpRequest::new(url.as_str()),
        None::<String>,
        "MDBList didn't answer",
    )?;
    Ok(Answer {
        status: response.status_code(),
        body: response.into_body(),
    })
}

/// A row for each list `setting` names, in the settings' order, asking MDBList through `send`.
///
/// # Errors
/// If the API key isn't set, or any list can't be read: a refresh shows every list or none.
fn rows(
    setting: impl Fn(&str) -> Result<Option<String>, Error>,
    send: impl FnMut(&Url) -> Result<Answer, Error>,
) -> Result<HomeRows, Error> {
    let text = |key| -> Result<Option<String>, Error> {
        Ok(setting(key)?
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()))
    };
    let mut api = Api {
        key: text(API_KEY)?.ok_or_else(|| Error::msg("the setting api_key isn't set"))?,
        send,
    };
    let mut rows = Vec::new();
    for key in LISTS {
        if let Some(list) = text(key)? {
            rows.push(api.row(&list)?);
        }
    }
    Ok(HomeRows {
        rows,
        ..HomeRows::default()
    })
}

/// MDBList's API, asked with the Admin's key through `send`.
struct Api<F> {
    key: String,
    send: F,
}

/// A list as MDBList describes it.
#[derive(Deserialize)]
struct List {
    id: u64,
    name: String,
}

/// A page of a list's items, its movies apart from its shows.
#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    movies: Vec<Item>,
    #[serde(default)]
    shows: Vec<Item>,
}

/// One title in a list.
#[derive(Deserialize)]
struct Item {
    /// Its TMDB id, which MDBList lacks for a few titles.
    id: Option<u64>,
    #[serde(default)]
    rank: u64,
}

impl<F: FnMut(&Url) -> Result<Answer, Error>> Api<F> {
    /// The row of the list `setting` names, of its first [`MOST_TITLES`] titles.
    fn row(&mut self, setting: &str) -> Result<HomeRow, Error> {
        let list: Vec<List> = self.get(&list_path(setting)?, setting, &[])?;
        let list = list.into_iter().next().ok_or_else(|| gone(setting))?;
        let key = list.id.to_string();
        Ok(HomeRow {
            titles: self.titles(&[&key, "items"], setting)?,
            key,
            heading: list.name,
            ttl_seconds: FRESH_FOR.as_secs().try_into()?,
            ..HomeRow::default()
        })
    }

    /// The first [`MOST_TITLES`] titles in the items at `path`, of the list `setting` names, a
    /// page at a time until a page comes back short.
    fn titles(&mut self, path: &[&str], setting: &str) -> Result<Vec<HomeRowTitle>, Error> {
        let mut titles = Vec::new();
        let mut offset = 0;
        while offset < MOST_TITLES {
            let limit = PAGE_ITEMS.min(MOST_TITLES - offset);
            let query = [("limit", limit.to_string()), ("offset", offset.to_string())];
            let page: Page = self.get(path, setting, &query)?;
            let mut items: Vec<_> = (page.movies.into_iter())
                .map(|item| (TitleKind::TITLE_KIND_MOVIE, item))
                .chain(
                    page.shows
                        .into_iter()
                        .map(|item| (TitleKind::TITLE_KIND_SHOW, item)),
                )
                .collect();
            let short = items.len() < limit;
            offset += items.len();
            // Ranks run across both media types, so merging by rank keeps the list's order.
            items.sort_by_key(|(_, item)| item.rank);
            titles.extend(
                items
                    .into_iter()
                    .filter_map(|(kind, item)| title(kind, item.id)),
            );
            if short {
                break;
            }
        }
        Ok(titles)
    }

    /// MDBList's answer at `path` under [`LISTS_URL`] with `query`, for the list `setting`
    /// names.
    ///
    /// # Errors
    /// If MDBList didn't answer, refused, or answered with something unexpected.
    fn get<T: for<'de> Deserialize<'de>>(
        &mut self,
        path: &[&str],
        setting: &str,
        query: &[(&str, String)],
    ) -> Result<T, Error> {
        let mut url = Url::parse(LISTS_URL)?;
        url.path_segments_mut()
            .map_err(|()| Error::msg("MDBList's API URL has no path"))?
            .pop_if_empty()
            .extend(path);
        url.query_pairs_mut()
            .append_pair("apikey", &self.key)
            .extend_pairs(query);
        let answer = (self.send)(&url)?;
        match answer.status {
            200..300 => json::from_slice(&answer.body).map_err(|err| {
                Error::msg(format!("MDBList answered with something unexpected: {err}"))
            }),
            401 => Err(Error::msg("MDBList refused the API key")),
            403 | 404 => Err(gone(setting)),
            status => Err(Error::msg(format!(
                "MDBList refused the request for the list {setting} with HTTP status {status}"
            ))),
        }
    }
}

/// The path under [`LISTS_URL`] of the list `setting` names by its address, such as
/// `https://mdblist.com/lists/alice/classics`, its owner and name, or its id.
fn list_path(setting: &str) -> Result<Vec<&str>, Error> {
    let path = setting
        .rsplit_once("/lists/")
        .map_or(setting, |(_, path)| path);
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if (1..=2).contains(&segments.len()) {
        Ok(segments)
    } else {
        Err(Error::msg(format!(
            "{setting} isn't an MDBList list's address or id"
        )))
    }
}

/// The refresh's error for a list MDBList won't show: it answers a gone and a private list alike.
fn gone(setting: &str) -> Error {
    Error::msg(format!("the MDBList list {setting} is gone or private"))
}

/// The Home row title for a list item of `kind`, or `None` without its TMDB id.
fn title(kind: TitleKind, tmdb_id: Option<u64>) -> Option<HomeRowTitle> {
    Some(HomeRowTitle {
        kind: kind.into(),
        id: TitleId {
            namespace: "tmdb".to_owned(),
            value: tmdb_id?.to_string(),
            ..TitleId::default()
        }
        .into(),
        ..HomeRowTitle::default()
    })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

#[cfg(test)]
mod fake_mdblist;
