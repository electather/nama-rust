//! The fanart.tv Installed Plugin: a title source serving only artwork lists, for Movies by their
//! TMDB or IMDb id and Shows by their TVDB id, and the images they name. The Server builds it for
//! wasm32 and bundles it in its binary.

use nama_pdk::{
    Artwork, ArtworkAnswer, ArtworkAsked, ArtworkKind, Image, ImageAnswer, ImageAsked, Manifest,
    TitleKind, TitleSource,
    extism_pdk::{Error, HttpRequest, HttpResponse, config, json},
    nama_plugin_proto::nama::plugin::v1::{SettingField, SettingKind, SettingValue, setting_value},
    services,
};
use serde::Deserialize;

const API_KEY: &str = "api_key";
const CLIENT_KEY: &str = "client_key";
const API_URL: &str = "api_url";
const ASSETS_URL: &str = "assets_url";

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, FnResult, Manifest, extism_pdk, plugin_fn};

    // ponytail: no `check_settings`, so a wrong key shows only when artwork is asked for; add
    // one asking fanart.tv for a known title if Admins trip over it.
    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let setting = |key: &str, label: &str, description: &str, kind: SettingKind| SettingField {
        key: key.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: kind.into(),
        ..SettingField::default()
    };
    let url = |key: &str, label: &str, description: &str, default: &str| SettingField {
        default_value: SettingValue {
            value: Some(setting_value::Value::TextValue(default.to_owned())),
            ..SettingValue::default()
        }
        .into(),
        ..setting(key, label, description, SettingKind::SETTING_KIND_URL)
    };
    Manifest {
        name: "fanart.tv".to_owned(),
        description: "Finds logos, backdrops, posters and other artwork on fanart.tv.".to_owned(),
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::TITLE_SOURCE)],
        title_namespace: "fanart".to_owned(),
        title_lookup_namespaces: vec!["tmdb".to_owned(), "imdb".to_owned(), "tvdb".to_owned()],
        allowed_hosts: vec![
            "webservice.fanart.tv".to_owned(),
            "assets.fanart.tv".to_owned(),
        ],
        settings: vec![
            SettingField {
                required: true,
                ..setting(
                    API_KEY,
                    "API key",
                    "A fanart.tv project API key, from the API page of your fanart.tv account.",
                    SettingKind::SETTING_KIND_SECRET,
                )
            },
            setting(
                CLIENT_KEY,
                "Personal API key",
                "Your personal fanart.tv API key, which gets you newer artwork sooner.",
                SettingKind::SETTING_KIND_SECRET,
            ),
            url(
                API_URL,
                "API URL",
                "Where fanart.tv's v3 API is. Change it only for a server of your own.",
                "https://webservice.fanart.tv/v3",
            ),
            url(
                ASSETS_URL,
                "Assets URL",
                "Where fanart.tv's images are. Change it only for a server of your own.",
                "https://assets.fanart.tv",
            ),
        ],
        ..nama_pdk::manifest("fanart", env!("CARGO_PKG_VERSION"))
    }
}

/// Serves fanart.tv's artwork.
struct Fanart;

impl TitleSource for Fanart {
    fn artwork(asked: ArtworkAsked) -> Result<ArtworkAnswer, Error> {
        let Some((path, lists)) = lookup(&asked)? else {
            return Ok(ArtworkAnswer {
                unsupported_id: true,
                ..ArtworkAnswer::default()
            });
        };
        let mut url = format!(
            "{}/{path}?api_key={}",
            setting(API_URL)?.trim_end_matches('/'),
            encoded(&setting(API_KEY)?),
        );
        if let Some(client_key) = config::get(CLIENT_KEY)?.filter(|key| !key.trim().is_empty()) {
            url = format!("{url}&client_key={}", encoded(client_key.trim()));
        }
        let Some(response) = accepted(send(&HttpRequest::new(url))?, "fanart.tv")? else {
            return Ok(ArtworkAnswer::default());
        };
        Ok(ArtworkAnswer {
            artwork: artwork(lists, &response.body())?,
            ..ArtworkAnswer::default()
        })
    }

    fn image(asked: ImageAsked) -> Result<ImageAnswer, Error> {
        // A path that isn't one keeps the request on the assets host.
        if !asked.path.starts_with('/') {
            return Ok(ImageAnswer::default());
        }
        let url = format!(
            "{}{}",
            setting(ASSETS_URL)?.trim_end_matches('/'),
            asked.path
        );
        let Some(response) = accepted(send(&HttpRequest::new(url))?, "fanart.tv's assets host")?
        else {
            return Ok(ImageAnswer::default());
        };
        Ok(ImageAnswer {
            image: Image {
                // The Server gives a Plugin no response headers, so the type comes from the path.
                content_type: content_type(&asked.path).to_owned(),
                bytes: response.body(),
                ..Image::default()
            }
            .into(),
            ..ImageAnswer::default()
        })
    }
}

nama_pdk::title_source!(Fanart, [artwork, image]);

/// One of fanart.tv's lists: its field, the kind of artwork in it, and the fixed width and height
/// fanart.tv documents for it, if any.
type List = (&'static str, ArtworkKind, Option<(u32, u32)>);

use ArtworkKind::{
    ARTWORK_KIND_BACKDROP as BACKDROP, ARTWORK_KIND_BANNER as BANNER,
    ARTWORK_KIND_CLEAR_ART as CLEAR_ART, ARTWORK_KIND_DISC as DISC, ARTWORK_KIND_LOGO as LOGO,
    ARTWORK_KIND_POSTER as POSTER, ARTWORK_KIND_THUMB as THUMB,
};

/// A Movie's lists, each HD one before its SD one, so the source's own order prefers HD.
const MOVIE: &[List] = &[
    ("hdmovielogo", LOGO, Some((800, 310))),
    ("movielogo", LOGO, Some((400, 155))),
    ("movieposter", POSTER, Some((1000, 1426))),
    ("moviebackground", BACKDROP, Some((1920, 1080))),
    ("hdmovieclearart", CLEAR_ART, Some((1000, 562))),
    ("movieart", CLEAR_ART, Some((500, 281))),
    ("moviebanner", BANNER, Some((1000, 185))),
    ("moviethumb", THUMB, Some((1000, 562))),
    ("moviedisc", DISC, Some((1000, 1000))),
];

/// A Show's lists, ordered as [`MOVIE`]'s. Its Seasons' lists are left out.
const SHOW: &[List] = &[
    ("hdtvlogo", LOGO, Some((800, 310))),
    ("clearlogo", LOGO, Some((400, 155))),
    ("tvposter", POSTER, Some((1000, 1426))),
    ("showbackground", BACKDROP, Some((1920, 1080))),
    ("hdclearart", CLEAR_ART, Some((1000, 562))),
    ("clearart", CLEAR_ART, Some((500, 281))),
    ("tvbanner", BANNER, Some((1000, 185))),
    ("tvthumb", THUMB, Some((500, 281))),
];

/// The API path and lists of the title `asked` names, such as `movies/949`, `None` if fanart.tv
/// can't look it up by that id.
///
/// # Errors
/// If there is no id.
fn lookup(asked: &ArtworkAsked) -> Result<Option<(String, &'static [List])>, Error> {
    let id = asked
        .id
        .as_option()
        .ok_or_else(|| Error::msg("fanart.tv was asked for no id"))?;
    let found = match (asked.kind.as_known(), id.namespace.as_str()) {
        (Some(TitleKind::TITLE_KIND_MOVIE), "tmdb" | "imdb") => Some(("movies", MOVIE)),
        (Some(TitleKind::TITLE_KIND_SHOW), "tvdb") => Some(("tv", SHOW)),
        _ => None,
    };
    Ok(found.map(|(section, lists)| (format!("{section}/{}", encoded(&id.value)), lists)))
}

/// One image in a fanart.tv list.
#[derive(Deserialize)]
struct Listed {
    url: String,
    #[serde(default)]
    lang: String,
    #[serde(default)]
    likes: String,
}

/// The artwork in fanart.tv's answer `body`, in the order of `lists`.
fn artwork(lists: &[List], body: &[u8]) -> Result<Vec<Artwork>, Error> {
    let answer: json::Map<String, json::Value> =
        json::from_slice(body).map_err(|err| unexpected(&err))?;
    let mut artwork = Vec::new();
    for &(field, kind, size) in lists {
        let Some(listed) = answer.get(field) else {
            continue;
        };
        let listed: Vec<Listed> =
            json::from_value(listed.clone()).map_err(|err| unexpected(&err))?;
        artwork.extend(listed.into_iter().filter_map(|image| {
            Some(Artwork {
                kind: kind.into(),
                path: asset_path(&image.url)?,
                language: Some(image.lang).filter(|lang| !lang.is_empty() && lang != "00"),
                width: size.map(|(width, _)| width),
                height: size.map(|(_, height)| height),
                votes: image.likes.parse().ok(),
                ..Artwork::default()
            })
        }));
    }
    Ok(artwork)
}

fn unexpected(err: &json::Error) -> Error {
    Error::msg(format!(
        "fanart.tv answered with something unexpected: {err}"
    ))
}

/// The path of the asset at `url`, such as `/fanart/movies/949/hdmovielogo/heat.png`, which the
/// `image` export fetches from the assets URL setting.
fn asset_path(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    rest.split_once('/').map(|(_, path)| format!("/{path}"))
}

/// The setting `key`, which the Server sets before every call.
fn setting(key: &str) -> Result<String, Error> {
    config::get(key)?.ok_or_else(|| Error::msg(format!("the setting {key} isn't set")))
}

fn send(request: &HttpRequest) -> Result<HttpResponse, Error> {
    nama_pdk::http_request(request, None::<String>, "fanart.tv didn't answer")
}

/// `response` if `service` took the request, `None` for a 404, else an error naming the status
/// and nothing of the request, which holds the key.
fn accepted(response: HttpResponse, service: &str) -> Result<Option<HttpResponse>, Error> {
    match response.status_code() {
        200..300 => Ok(Some(response)),
        404 => Ok(None),
        status => Err(Error::msg(format!(
            "{service} refused the request with HTTP status {status}"
        ))),
    }
}

/// `text` percent-encoded for a URL.
fn encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            byte => format!("%{byte:02X}"),
        })
        .collect()
}

fn content_type(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/jpeg",
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
