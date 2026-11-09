//! Talking to TMDB over HTTPS, as a [`MetadataProvider`].

use std::time::Duration;

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
    metadata::{
        Candidate, Episode, ExternalId, Image, MetadataError, MetadataProvider, Movie, SearchQuery,
        Season, Show, TitleKind,
    },
};
use serde::de::DeserializeOwned;
use url::Url;

use crate::{
    Endpoints,
    wire::{
        EpisodeDetails, Found, MovieDetails, MovieResult, Page, SeasonDetails, ShowDetails,
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
        let connector = HttpsConnectorBuilder::new()
            .with_provider_and_native_roots(rustls::crypto::ring::default_provider())?
            .https_or_http()
            .enable_http1()
            .build();
        Ok(Self {
            http: HyperClient::builder(TokioExecutor::new()).build(connector),
            endpoints,
            api_key: api_key.to_owned(),
            language: language.to_owned(),
        })
    }

    async fn api<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, MetadataError> {
        let mut url = Url::parse(&format!("{}{path}", self.endpoints.api)).map_err(failed)?;
        url.query_pairs_mut()
            .append_pair("api_key", &self.api_key)
            .append_pair("language", &self.language)
            .extend_pairs(params);
        let (_, body) = self.get(url).await?;
        serde_json::from_slice(&body).map_err(|err| failed(format!("TMDB sent {path}: {err}")))
    }

    /// The TMDB id of the `kind` of title `id` names, found with `/find` if it's another source's.
    async fn find(&self, kind: TitleKind, id: &ExternalId) -> Result<i64, MetadataError> {
        let (source, id) = match id {
            ExternalId::Tmdb(id) => return Ok(*id),
            ExternalId::Tvdb(id) => ("tvdb_id", id.to_string()),
            // The id goes in the path, so only an IMDb id's own shape is sent.
            ExternalId::Imdb(id) if is_imdb_id(id) => ("imdb_id", id.clone()),
            ExternalId::Imdb(_) => return Err(MetadataError::NotFound),
        };
        let found: Found = self
            .api(&format!("/find/{id}"), &[("external_source", source)])
            .await?;
        let results = match kind {
            TitleKind::Movie => found.movie_results,
            TitleKind::Show => found.tv_results,
        };
        results
            .first()
            .map(|title| title.id)
            .ok_or(MetadataError::NotFound)
    }

    async fn details<T: DeserializeOwned>(&self, path: &str) -> Result<T, MetadataError> {
        self.api(path, &[("append_to_response", "external_ids")])
            .await
    }

    /// GETs `url`, returning the content type and body of a successful response.
    async fn get(&self, url: Url) -> Result<(Option<String>, Bytes), MetadataError> {
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
                Ok((content_type, body))
            }
            StatusCode::NOT_FOUND => Err(MetadataError::NotFound),
            StatusCode::UNAUTHORIZED => Err(failed("TMDB rejected the API key")),
            StatusCode::TOO_MANY_REQUESTS => Err(failed("TMDB is limiting how fast we ask")),
            status => Err(failed(format!("TMDB answered {status}"))),
        }
    }
}

/// `tt` and digits, as IMDb's title ids are.
fn is_imdb_id(id: &str) -> bool {
    id.strip_prefix("tt")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// The TMDB id in `id`, the only kind TMDB looks Seasons and Episodes up by.
fn tmdb_id(id: &ExternalId) -> Result<i64, MetadataError> {
    match *id {
        ExternalId::Tmdb(id) => Ok(id),
        ExternalId::Imdb(_) | ExternalId::Tvdb(_) => Err(MetadataError::UnsupportedId(id.clone())),
    }
}

/// A failure worth trying again later, such as TMDB being unreachable.
fn failed(error: impl Into<nama_plugin_api::PluginError>) -> MetadataError {
    MetadataError::Failed(error.into())
}

impl MetadataProvider for Client {
    fn search<'a>(
        &'a self,
        query: &'a SearchQuery,
    ) -> BoxFuture<'a, Result<Vec<Candidate>, MetadataError>> {
        Box::pin(async move {
            let year = query.year.map(|year| year.to_string());
            let (path, year_param) = match query.kind {
                TitleKind::Movie => ("/search/movie", "year"),
                TitleKind::Show => ("/search/tv", "first_air_date_year"),
            };
            let mut params = vec![("query", query.title.as_str()), ("include_adult", "false")];
            params.extend(year.as_deref().map(|year| (year_param, year)));
            Ok(match query.kind {
                TitleKind::Movie => {
                    let page: Page<MovieResult> = self.api(path, &params).await?;
                    page.results.into_iter().map(Candidate::from).collect()
                }
                TitleKind::Show => {
                    let page: Page<ShowResult> = self.api(path, &params).await?;
                    page.results.into_iter().map(Candidate::from).collect()
                }
            })
        })
    }

    fn movie<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Movie, MetadataError>> {
        Box::pin(async move {
            let id = self.find(TitleKind::Movie, id).await?;
            let movie: MovieDetails = self.details(&format!("/movie/{id}")).await?;
            Ok(movie.into())
        })
    }

    fn show<'a>(&'a self, id: &'a ExternalId) -> BoxFuture<'a, Result<Show, MetadataError>> {
        Box::pin(async move {
            let id = self.find(TitleKind::Show, id).await?;
            let show: ShowDetails = self.details(&format!("/tv/{id}")).await?;
            Ok(show.into())
        })
    }

    fn season<'a>(
        &'a self,
        show: &'a ExternalId,
        season: u32,
    ) -> BoxFuture<'a, Result<Season, MetadataError>> {
        Box::pin(async move {
            let show = tmdb_id(show)?;
            let season: SeasonDetails =
                self.details(&format!("/tv/{show}/season/{season}")).await?;
            Ok(season.into())
        })
    }

    fn episode<'a>(
        &'a self,
        show: &'a ExternalId,
        season: u32,
        episode: u32,
    ) -> BoxFuture<'a, Result<Episode, MetadataError>> {
        Box::pin(async move {
            let show = tmdb_id(show)?;
            let episode: EpisodeDetails = self
                .details(&format!("/tv/{show}/season/{season}/episode/{episode}"))
                .await?;
            Ok(episode.into())
        })
    }

    fn image<'a>(&'a self, path: &'a str) -> BoxFuture<'a, Result<Image, MetadataError>> {
        Box::pin(async move {
            // TMDB's image paths are a single `/name.ext`; anything else isn't one of its images.
            let name = path.strip_prefix('/').unwrap_or(path);
            if name.is_empty() || name.contains(['/', '\\', '?', '#']) || name.contains("..") {
                return Err(MetadataError::NotFound);
            }
            let url = Url::parse(&format!("{}/{IMAGE_SIZE}/{name}", self.endpoints.images))
                .map_err(failed)?;
            let (content_type, bytes) = self.get(url).await?;
            Ok(Image {
                content_type: content_type.unwrap_or_else(|| "image/jpeg".to_owned()),
                bytes: bytes.to_vec(),
            })
        })
    }
}
