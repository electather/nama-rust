//! Talking to TMDB over HTTPS, as a [`TitleSource`].

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http::{Request, StatusCode, header};
use http_body_util::{BodyExt, Empty, Limited};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client as HyperClient, connect::HttpConnector},
    rt::TokioExecutor,
};
use nama_plugin_api::{
    BoxFuture,
    contract::{
        ArtworkAnswer, ArtworkAsked, Candidate, EpisodeAnswer, EpisodeAsked, Image, ImageAnswer,
        ImageAsked, MovieAnswer, SearchQuery, SearchResults, SeasonAnswer, SeasonAsked, ShowAnswer,
        TitleAsked, TitleId, TitleKind,
    },
    metadata::{IMDB, MetadataError, TMDB, TVDB, TitleSource},
};
use serde::de::DeserializeOwned;
use url::Url;

use crate::{
    Endpoints,
    wire::{
        EpisodeDetails, Found, Images, MovieDetails, MovieResult, Page, SeasonDetails, ShowDetails,
        ShowResult,
    },
};

const TIMEOUT: Duration = Duration::from_secs(20);

/// The largest response read, so a misbehaving server can't exhaust memory. Original-size
/// backdrops are a few megabytes.
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

const IMAGE_SIZE: &str = "original";

pub(crate) struct Client {
    http: HyperClient<HttpsConnector<HttpConnector>, Empty<Bytes>>,
    endpoints: Endpoints,
    api_key: String,
    language: String,
}

impl Client {
    pub(crate) fn new(
        endpoints: Endpoints,
        api_key: &str,
        language: &str,
    ) -> std::io::Result<Self> {
        let provider = rustls::crypto::ring::default_provider();
        let builder = HttpsConnectorBuilder::new();
        // A plain-HTTP stand-in needs no trust roots, and loading them can fail: macOS's trust
        // settings answer I/O error (-36) after about 20 s when many test processes ask at once.
        let builder = if [&endpoints.api, &endpoints.images]
            .iter()
            .all(|url| url.starts_with("http://"))
        {
            let tls = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                .with_safe_default_protocol_versions()
                .map_err(std::io::Error::other)?
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth();
            builder.with_tls_config(tls)
        } else {
            builder.with_provider_and_native_roots(provider)?
        };
        let connector = builder.https_or_http().enable_http1().build();
        Ok(Self {
            http: HyperClient::builder(TokioExecutor::new()).build(connector),
            endpoints,
            api_key: api_key.to_owned(),
            language: language.to_owned(),
        })
    }

    /// The JSON at `path` in the configured language, or `None` where TMDB has nothing there.
    async fn api<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Option<T>, MetadataError> {
        self.api_in(Some(&self.language), path, params).await
    }

    /// The JSON at `path`, or `None` where TMDB has nothing there. With no `language`, TMDB
    /// answers in every language, which is what an images list needs.
    async fn api_in<T: DeserializeOwned>(
        &self,
        language: Option<&str>,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Option<T>, MetadataError> {
        let mut url = Url::parse(&format!("{}{path}", self.endpoints.api)).map_err(failed)?;
        url.query_pairs_mut()
            .append_pair("api_key", &self.api_key)
            .extend_pairs(language.map(|language| ("language", language)))
            .extend_pairs(params);
        let Some((_, body)) = self.get(url).await? else {
            return Ok(None);
        };
        serde_json::from_slice(&body)
            .map(Some)
            .map_err(|err| failed(format!("TMDB sent {path}: {err}")))
    }

    /// The TMDB id of the `kind` of title `id` names, found with `/find` if it's another source's.
    async fn find(&self, kind: TitleKind, id: &TitleId) -> Result<Lookup, MetadataError> {
        let source = match id.namespace.as_str() {
            TMDB => return Ok(tmdb_id(id)),
            // The id goes in the path, so only a number or an IMDb id's own shape is sent.
            TVDB if is_number(&id.value) => "tvdb_id",
            IMDB if is_imdb_id(&id.value) => "imdb_id",
            TVDB | IMDB => return Ok(Lookup::Missing),
            _ => return Ok(Lookup::Unsupported),
        };
        let found: Option<Found> = self
            .api(
                &format!("/find/{}", id.value),
                &[("external_source", source)],
            )
            .await?;
        let results = match (found, kind) {
            (Some(found), TitleKind::TITLE_KIND_MOVIE) => found.movie_results,
            (Some(found), TitleKind::TITLE_KIND_SHOW) => found.tv_results,
            _ => Vec::new(),
        };
        Ok(results
            .first()
            .map_or(Lookup::Missing, |title| Lookup::Found(title.id)))
    }

    async fn details<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, MetadataError> {
        self.api(path, &[("append_to_response", "external_ids")])
            .await
    }

    /// The details of the Movie or Show at `path`, with `ratings`, the field its ratings by
    /// country are appended as.
    async fn title_details<T: DeserializeOwned>(
        &self,
        path: &str,
        ratings: &str,
    ) -> Result<Option<T>, MetadataError> {
        let append = format!("external_ids,{ratings}");
        self.api(path, &[("append_to_response", append.as_str())])
            .await
    }

    /// The country part of the Language setting, `US` of `en-US` or `TW` of `zh-Hant-TW`, whose
    /// Content Ratings count, if it names one.
    fn country(&self) -> Option<&str> {
        // A region subtag is the two-letter one after the language (RFC 5646).
        (self.language.split('-').skip(1)).find(|subtag| subtag.len() == 2)
    }

    /// GETs `url`, returning the content type and body of a successful response, or `None` for
    /// one TMDB doesn't have.
    async fn get(&self, url: Url) -> Result<Option<(Option<String>, Bytes)>, MetadataError> {
        let request = Request::get(url.as_str())
            .header(header::ACCEPT, "application/json, image/*")
            .header(
                header::USER_AGENT,
                concat!("nama-plugin-tmdb/", env!("CARGO_PKG_VERSION")),
            )
            .body(Empty::new())
            .map_err(failed)?;
        let response = async {
            let response = self.http.request(request).await.map_err(failed)?;
            let (parts, body) = response.into_parts();
            let body = Limited::new(body, MAX_RESPONSE_BYTES)
                .collect()
                .await
                .map_err(|err| failed(err.to_string()))?
                .to_bytes();
            Ok::<_, MetadataError>((parts, body))
        };
        let (parts, body) = tokio::time::timeout(TIMEOUT, response)
            .await
            .map_err(|_elapsed| failed(format!("TMDB didn't answer within {TIMEOUT:?}")))??;
        match parts.status {
            StatusCode::OK => {
                let content_type = parts
                    .headers
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                Ok(Some((content_type, body)))
            }
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::UNAUTHORIZED => Err(failed("TMDB rejected the API key")),
            StatusCode::TOO_MANY_REQUESTS => Err(failed("TMDB is limiting how fast we ask")),
            status => Err(failed(format!("TMDB answered {status}"))),
        }
    }
}

/// `tt` and digits, as IMDb's title ids are.
fn is_imdb_id(id: &str) -> bool {
    id.strip_prefix("tt").is_some_and(is_number)
}

fn is_number(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// What an asked id comes to at TMDB.
enum Lookup {
    /// The TMDB id of the title.
    Found(i64),
    /// TMDB has no such title.
    Missing,
    /// TMDB can't look titles up by the id's namespace.
    Unsupported,
}

/// The TMDB id in `id`, the only kind TMDB looks Seasons and Episodes up by.
fn tmdb_id(id: &TitleId) -> Lookup {
    if id.namespace != TMDB {
        return Lookup::Unsupported;
    }
    id.value.parse().map_or(Lookup::Missing, Lookup::Found)
}

/// The Candidates on a search's page; none where TMDB has no page.
fn candidates<T: Into<Candidate>>(page: Option<Page<T>>) -> Vec<Candidate> {
    page.map_or_else(Vec::new, |page| {
        page.results.into_iter().map(Into::into).collect()
    })
}

/// A failure worth trying again later, such as TMDB being unreachable.
fn failed(error: impl Into<nama_plugin_api::PluginError>) -> MetadataError {
    MetadataError::Failed(error.into())
}

/// The namespaces TMDB looks titles up by, which its Manifest declares too.
pub(crate) fn looks_up() -> Vec<String> {
    vec![IMDB.to_owned(), TVDB.to_owned()]
}

impl TitleSource for Client {
    fn namespace(&self) -> &'static str {
        TMDB
    }

    fn lookup_namespaces(&self) -> Vec<String> {
        looks_up()
    }

    fn search<'a>(
        &'a self,
        query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<SearchResults, MetadataError>> {
        Box::pin(async move {
            let year = query.year.map(|year| year.to_string());
            let (path, year_param) = match query.kind.as_known() {
                Some(TitleKind::TITLE_KIND_MOVIE) => ("/search/movie", "year"),
                Some(TitleKind::TITLE_KIND_SHOW) => ("/search/tv", "first_air_date_year"),
                // A kind TMDB doesn't know finds nothing.
                _ => return Ok(SearchResults::default()),
            };
            let mut params = vec![("query", query.title.as_str()), ("include_adult", "false")];
            params.extend(year.as_deref().map(|year| (year_param, year)));
            let candidates = if query.kind == TitleKind::TITLE_KIND_MOVIE {
                candidates::<MovieResult>(self.api(path, &params).await?)
            } else {
                candidates::<ShowResult>(self.api(path, &params).await?)
            };
            Ok(SearchResults {
                candidates,
                ..Default::default()
            })
        })
    }

    fn movie<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<MovieAnswer, MetadataError>> {
        Box::pin(async move {
            let id = match self.find(TitleKind::TITLE_KIND_MOVIE, &asked.id).await? {
                Lookup::Found(id) => id,
                Lookup::Missing => return Ok(MovieAnswer::default()),
                Lookup::Unsupported => {
                    return Ok(MovieAnswer {
                        unsupported_id: true,
                        ..Default::default()
                    });
                }
            };
            let movie: Option<MovieDetails> = self
                .title_details(&format!("/movie/{id}"), "release_dates")
                .await?;
            Ok(MovieAnswer {
                movie: movie.map(|movie| movie.into_movie(self.country())).into(),
                ..Default::default()
            })
        })
    }

    fn show<'a>(
        &'a self,
        asked: &'a TitleAsked,
    ) -> BoxFuture<'a, Result<ShowAnswer, MetadataError>> {
        Box::pin(async move {
            let id = match self.find(TitleKind::TITLE_KIND_SHOW, &asked.id).await? {
                Lookup::Found(id) => id,
                Lookup::Missing => return Ok(ShowAnswer::default()),
                Lookup::Unsupported => {
                    return Ok(ShowAnswer {
                        unsupported_id: true,
                        ..Default::default()
                    });
                }
            };
            let show: Option<ShowDetails> = self
                .title_details(&format!("/tv/{id}"), "content_ratings")
                .await?;
            Ok(ShowAnswer {
                show: show.map(|show| show.into_show(self.country())).into(),
                ..Default::default()
            })
        })
    }

    fn season<'a>(
        &'a self,
        asked: &'a SeasonAsked,
    ) -> BoxFuture<'a, Result<SeasonAnswer, MetadataError>> {
        Box::pin(async move {
            let show = match tmdb_id(&asked.show) {
                Lookup::Found(show) => show,
                Lookup::Missing => return Ok(SeasonAnswer::default()),
                Lookup::Unsupported => {
                    return Ok(SeasonAnswer {
                        unsupported_id: true,
                        ..Default::default()
                    });
                }
            };
            let path = format!("/tv/{show}/season/{}", asked.season);
            let season: Option<SeasonDetails> = self.details(&path).await?;
            Ok(SeasonAnswer {
                season: season.map(Into::into).into(),
                ..Default::default()
            })
        })
    }

    fn episode<'a>(
        &'a self,
        asked: &'a EpisodeAsked,
    ) -> BoxFuture<'a, Result<EpisodeAnswer, MetadataError>> {
        Box::pin(async move {
            let show = match tmdb_id(&asked.show) {
                Lookup::Found(show) => show,
                Lookup::Missing => return Ok(EpisodeAnswer::default()),
                Lookup::Unsupported => {
                    return Ok(EpisodeAnswer {
                        unsupported_id: true,
                        ..Default::default()
                    });
                }
            };
            let path = format!(
                "/tv/{show}/season/{}/episode/{}",
                asked.season, asked.episode
            );
            let episode: Option<EpisodeDetails> = self.details(&path).await?;
            Ok(EpisodeAnswer {
                episode: episode.map(Into::into).into(),
                ..Default::default()
            })
        })
    }

    fn image<'a>(
        &'a self,
        asked: &'a ImageAsked,
    ) -> BoxFuture<'a, Result<ImageAnswer, MetadataError>> {
        Box::pin(async move {
            // TMDB's image paths are a single `/name.ext`; anything else isn't one of its images.
            let name = asked.path.strip_prefix('/').unwrap_or(&asked.path);
            if name.is_empty() || name.contains(['/', '\\', '?', '#']) || name.contains("..") {
                return Ok(ImageAnswer::default());
            }
            let url = Url::parse(&format!("{}/{IMAGE_SIZE}/{name}", self.endpoints.images))
                .map_err(failed)?;
            let image = self.get(url).await?.map(|(content_type, bytes)| Image {
                content_type: content_type.unwrap_or_else(|| "image/jpeg".to_owned()),
                bytes: bytes.to_vec(),
                ..Default::default()
            });
            Ok(ImageAnswer {
                image: image.into(),
                ..Default::default()
            })
        })
    }

    fn artwork<'a>(
        &'a self,
        asked: &'a ArtworkAsked,
    ) -> BoxFuture<'a, Result<ArtworkAnswer, MetadataError>> {
        Box::pin(async move {
            let (kind, section) = match asked.kind.as_known() {
                Some(kind @ TitleKind::TITLE_KIND_MOVIE) => (kind, "movie"),
                Some(kind @ TitleKind::TITLE_KIND_SHOW) => (kind, "tv"),
                // A kind TMDB doesn't know has no artwork.
                _ => return Ok(ArtworkAnswer::default()),
            };
            let id = match self.find(kind, &asked.id).await? {
                Lookup::Found(id) => id,
                Lookup::Missing => return Ok(ArtworkAnswer::default()),
                Lookup::Unsupported => {
                    return Ok(ArtworkAnswer {
                        unsupported_id: true,
                        ..Default::default()
                    });
                }
            };
            let path = format!("/{section}/{id}/images");
            let images: Option<Images> = self.api_in(None, &path, &[]).await?;
            Ok(ArtworkAnswer {
                artwork: images.map_or_default(Vec::from),
                ..Default::default()
            })
        })
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
