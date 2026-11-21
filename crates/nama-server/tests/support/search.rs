//! Calling `SearchService.Search` the way a client does.

use connectrpc::{
    ConnectError,
    client::{ClientConfig, HttpClient},
};
use nama_proto::nama::v1::{
    SearchRequest, SearchResponse, SearchServiceClient, TitleKind, search_response,
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
