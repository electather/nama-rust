//! An in-process stand-in for MDBList's API, so the tests need no network and no API key. It
//! serves its lists by id or by owner and name, refusing a request without [`VALID_KEY`].

use nama_pdk::{
    Response,
    extism_pdk::{
        HttpRequest,
        json::{self, Value, json},
    },
};
use url::Url;

/// The API key the fake takes.
pub(super) const VALID_KEY: &str = "fake-api-key";

/// One list the fake serves.
pub(super) struct FakeList {
    pub(super) id: u64,
    pub(super) owner: &'static str,
    pub(super) slug: &'static str,
    pub(super) name: &'static str,
    pub(super) private: bool,
    /// Its items in rank order, each its media type, `movie` or `show`, and TMDB id.
    pub(super) items: Vec<(&'static str, u64)>,
}

impl FakeList {
    /// A public list owned by `alice`.
    pub(super) fn new(id: u64, slug: &'static str, name: &'static str) -> Self {
        Self {
            id,
            owner: "alice",
            slug,
            name,
            private: false,
            items: Vec::new(),
        }
    }
}

/// MDBList's API in memory.
#[derive(Default)]
pub(super) struct FakeMdbList {
    pub(super) lists: Vec<FakeList>,
}

impl FakeMdbList {
    /// MDBList's answer to a GET of `request`'s URL.
    pub(super) fn answer(&self, request: &HttpRequest) -> Response {
        let Ok(url) = Url::parse(&request.url) else {
            return answer(400, &json!({ "error": "Invalid URL" }));
        };
        let key = url.query_pairs().find(|(name, _)| name == "apikey");
        if url.host_str() != Some("api.mdblist.com") || key.is_none_or(|(_, key)| key != VALID_KEY)
        {
            return answer(401, &json!({ "error": "Invalid API key" }));
        }
        let segments: Vec<_> = url.path_segments().into_iter().flatten().collect();
        let (list, items) = match segments.as_slice() {
            ["lists", id, "items"] => (self.by_id(id), true),
            ["lists", id] => (self.by_id(id), false),
            ["lists", owner, slug] => (
                self.lists
                    .iter()
                    .find(|list| list.owner == *owner && list.slug == *slug),
                false,
            ),
            _ => (None, false),
        };
        match list {
            None => answer(404, &json!({ "error": "List not found" })),
            Some(list) if list.private => answer(403, &json!({ "error": "List is private" })),
            Some(list) if items => answer(200, &page(list, &url)),
            Some(list) => answer(
                200,
                &json!([{ "id": list.id, "name": list.name, "slug": list.slug }]),
            ),
        }
    }

    fn by_id(&self, id: &str) -> Option<&FakeList> {
        self.lists.iter().find(|list| list.id.to_string() == id)
    }
}

/// The page of `list`'s items the `limit` and `offset` in `url` ask for, its movies apart from
/// its shows, as MDBList answers.
fn page(list: &FakeList, url: &Url) -> Value {
    let number = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .and_then(|(_, value)| value.parse::<usize>().ok())
    };
    let offset = number("offset").unwrap_or(0);
    let limit = number("limit").unwrap_or(usize::MAX);
    let item = |(rank, &(mediatype, id)): (usize, &(&str, u64))| json!({ "id": id, "rank": rank + 1, "mediatype": mediatype, "title": "A title" });
    let paged = || list.items.iter().enumerate().skip(offset).take(limit);
    let of = |kind: &'static str| -> Vec<Value> {
        paged()
            .filter(|(_, (mediatype, _))| *mediatype == kind)
            .map(item)
            .collect()
    };
    json!({ "movies": of("movie"), "shows": of("show") })
}

fn answer(status: u16, body: &Value) -> Response {
    Response::new(status, json::to_vec(body).unwrap_or_default())
}
