//! Calling `SearchService.Search` the way a client does.

use connectrpc::{
    ConnectError,
    client::{ClientConfig, HttpClient},
};
use nama_proto::nama::v1::{
    MovieHit, SearchRequest, SearchResponse, SearchServiceClient, ShowHit, TitleKind, hit,
    search_response,
};

/// Every part a search for `query` of `kind` streams, in order, as whoever `who` calls the Server
/// as.
pub(crate) async fn search(
    who: &ClientConfig,
    query: &str,
    kind: TitleKind,
) -> Result<Vec<search_response::Part>, ConnectError> {
    let request = SearchRequest {
        query: query.to_owned(),
        kind: kind.into(),
        ..Default::default()
    };
    let client = SearchServiceClient::new(HttpClient::plaintext(), who.clone());
    let mut stream = client.search(request).await?;
    let mut parts = Vec::new();
    while let Some(response) = stream.message::<SearchResponse>().await? {
        parts.extend(response.to_owned_message().part);
    }
    Ok(parts)
}

/// The Movies a search for `query` finds, the Libraries' and then the title source's, as whoever
/// `who` calls the Server as.
pub(crate) async fn movies(who: &ClientConfig, query: &str) -> Vec<MovieHit> {
    (titles(who, query, TitleKind::TITLE_KIND_MOVIE).await)
        .filter_map(|title| match title {
            hit::Title::Movie(movie) => Some(*movie),
            hit::Title::Show(_) | hit::Title::Episode(_) => None,
        })
        .collect()
}

/// The Shows a search for `query` finds, as [`movies`] finds Movies.
pub(crate) async fn shows(who: &ClientConfig, query: &str) -> Vec<ShowHit> {
    (titles(who, query, TitleKind::TITLE_KIND_SHOW).await)
        .filter_map(|title| match title {
            hit::Title::Show(show) => Some(*show),
            hit::Title::Movie(_) | hit::Title::Episode(_) => None,
        })
        .collect()
}

/// Every title a search for `query` of `kind` streams, batch after batch.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn titles(
    who: &ClientConfig,
    query: &str,
    kind: TitleKind,
) -> impl Iterator<Item = hit::Title> {
    (search(who, query, kind).await.unwrap().into_iter())
        .flat_map(|part| match part {
            search_response::Part::Hits(hits) => hits.hits,
            search_response::Part::Done(_) => Vec::new(),
        })
        .filter_map(|hit| hit.title)
}
