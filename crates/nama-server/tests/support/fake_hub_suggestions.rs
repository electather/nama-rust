//! What a fake Hub suggests to the Linked Server, the same whatever it is asked about: each title
//! by an id in the first namespace the Server asked in, or by TMDB id for Nama's charts.

use std::sync::{Arc, Mutex};

use axum::{Router, http::HeaderMap};
use connectrpc::ConnectError;
use nama_hub_proto::nama::hub::v1::{
    ExternalId, ListMostWatchlistedTitlesRequest, ListMostWatchlistedTitlesResponse,
    ListRecommendedTitlesRequest, ListRecommendedTitlesResponse, ListSimilarTitlesRequest,
    ListSimilarTitlesResponse, ListTrendingTitlesRequest, ListTrendingTitlesResponse,
    RecommendedTitle, SimilarTitle, TitleKind, TrendingTitle, WatchlistedTitle,
};
use ring::signature::Ed25519KeyPair;

use super::{State, caller, unary};

/// `app`, answering what the Hub suggests too.
pub(super) fn routes(app: Router, state: &Arc<Mutex<State>>, key: &Arc<Ed25519KeyPair>) -> Router {
    let linked = "/nama.hub.v1.LinkedServerService";
    app.route(
        &format!("{linked}/ListSimilarTitles"),
        unary(state, key, similar),
    )
    .route(
        &format!("{linked}/ListRecommendedTitles"),
        unary(state, key, recommended),
    )
    .route(
        &format!("{linked}/ListTrendingTitles"),
        unary(state, key, trending),
    )
    .route(
        &format!("{linked}/ListMostWatchlistedTitles"),
        unary(state, key, most_watchlisted),
    )
}

fn id(namespace: &str, value: &str) -> ExternalId {
    ExternalId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        ..Default::default()
    }
}

/// `604`, like any title, in the namespace of the first id asked about.
fn similar(
    state: &mut State,
    headers: &HeaderMap,
    request: &ListSimilarTitlesRequest,
    _: &Ed25519KeyPair,
) -> Result<ListSimilarTitlesResponse, ConnectError> {
    caller(state, headers)?;
    let namespace = request.ids.first().map_or("", |id| id.namespace.as_str());
    Ok(ListSimilarTitlesResponse {
        titles: vec![SimilarTitle {
            id: id(namespace, "604").into(),
            weight: 0.9,
            ..Default::default()
        }],
        ..Default::default()
    })
}

/// The Movie `604` and the Show `1396` for any User, in the first namespace asked, and a title of
/// no kind.
fn recommended(
    state: &mut State,
    headers: &HeaderMap,
    request: &ListRecommendedTitlesRequest,
    _: &Ed25519KeyPair,
) -> Result<ListRecommendedTitlesResponse, ConnectError> {
    caller(state, headers)?;
    let namespace = request.namespaces.first().map_or("", String::as_str);
    let title = |kind: TitleKind, value, weight| RecommendedTitle {
        kind: kind.into(),
        id: id(namespace, value).into(),
        weight,
        ..Default::default()
    };
    Ok(ListRecommendedTitlesResponse {
        titles: vec![
            title(TitleKind::TITLE_KIND_MOVIE, "604", 0.8),
            title(TitleKind::TITLE_KIND_SHOW, "1396", 0.4),
            title(TitleKind::TITLE_KIND_UNSPECIFIED, "1", 0.1),
        ],
        ..Default::default()
    })
}

/// The Movie `tmdb-603`.
fn trending(
    state: &mut State,
    headers: &HeaderMap,
    _: &ListTrendingTitlesRequest,
    _: &Ed25519KeyPair,
) -> Result<ListTrendingTitlesResponse, ConnectError> {
    caller(state, headers)?;
    Ok(ListTrendingTitlesResponse {
        titles: vec![TrendingTitle {
            id: id("tmdb", "603").into(),
            weight: 0.7,
            ..Default::default()
        }],
        ..Default::default()
    })
}

/// The Show `tmdb-1396`, then the Movie `tmdb-603`.
fn most_watchlisted(
    state: &mut State,
    headers: &HeaderMap,
    _: &ListMostWatchlistedTitlesRequest,
    _: &Ed25519KeyPair,
) -> Result<ListMostWatchlistedTitlesResponse, ConnectError> {
    caller(state, headers)?;
    let title = |kind: TitleKind, value, weight| WatchlistedTitle {
        kind: kind.into(),
        id: id("tmdb", value).into(),
        weight,
        ..Default::default()
    };
    Ok(ListMostWatchlistedTitlesResponse {
        titles: vec![
            title(TitleKind::TITLE_KIND_SHOW, "1396", 1.0),
            title(TitleKind::TITLE_KIND_MOVIE, "603", 0.5),
        ],
        ..Default::default()
    })
}
