//! Finding titles: `nama.v1.SearchService` (ADR 0080).

use std::collections::HashSet;

use buffa::EnumValue;
use connectrpc::{
    ConnectError, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::{
    StreamExt,
    future::{self, OptionFuture},
    stream,
};
use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::{
    self as proto, Done, Hits, RefusalReason, SearchRequest, SearchResponse, SearchService,
    TitleStatus, search_response::Part,
};

use crate::{
    browse::Browse,
    library::{
        Candidate, Downloaded, Found, FoundEpisode, FoundMovie, FoundShow, ImageSources, Role,
        ShownImages, TitleImage, TitleKind,
    },
    requests::{self, Requests, ShowToRequest, year_of},
};

use super::{
    caller, database_failed, external_id_to_proto, proto_number, refuse,
    requests::{searched_show_to_proto, searched_to_proto},
    title_kind_from_proto,
};

/// How many results a batch holds when the request doesn't say.
const DEFAULT_LIMIT: usize = 20;

/// The most results a batch holds, whatever the request says.
const MAX_LIMIT: usize = 100;

/// `nama.v1.SearchService`: finding titles, for any signed-in User.
pub(super) struct SearchServiceHandler {
    pub(super) browse: Browse,
    pub(super) requests: Requests,
    pub(super) images: ImageSources,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned stream is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl SearchService for SearchServiceHandler {
    async fn search(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SearchRequest>,
    ) -> ServiceResult<ServiceStream<SearchResponse>> {
        let viewer = caller(&ctx)?.user.clone();
        let request = request.to_owned_message();
        let query = searched_query(&request.query)?;
        let kind = searched_kind(request.kind)?;
        let limit = usize::try_from(request.limit)
            .ok()
            .filter(|&limit| limit > 0)
            .map_or(DEFAULT_LIMIT, |limit| limit.min(MAX_LIMIT));
        let found = self.browse.search(query, kind, limit).await;
        let found = found.map_err(database_failed)?;
        let images = self.images.shown().await.map_err(database_failed)?;
        let movie_ids = library_ids(&found, TitleKind::Movie);
        let show_ids = library_ids(&found, TitleKind::Show);
        let library = Hits {
            hits: (found.into_iter())
                .map(|found| hit_to_proto(found, &images))
                .collect(),
            ..Default::default()
        };
        let (requests, query) = (self.requests.clone(), query.to_owned());
        let after_library = async move {
            let asks = |of| kind.is_none_or(|kind| kind == of);
            let movies = OptionFuture::from(
                asks(TitleKind::Movie)
                    .then(|| requests.search_to_request(&viewer, &query, &movie_ids)),
            );
            let shows = OptionFuture::from(
                asks(TitleKind::Show)
                    .then(|| requests.search_shows_to_request(&viewer, &query, &show_ids)),
            );
            let (movies, shows) = future::join(movies, shows).await;
            to_request_parts([
                batch(movies, limit, |movie| movie_to_request_hit(movie, &images)),
                batch(shows, limit, |show| show_to_request_hit(show, &images)),
            ])
        };
        let stream: ServiceStream<SearchResponse> = Box::pin(
            stream::once(future::ready(Ok(response(library))))
                .chain(stream::once(after_library).flat_map(stream::iter)),
        );
        Response::ok(stream)
    }
}

/// The response of the stream that carries `part`.
fn response(part: impl Into<Part>) -> SearchResponse {
    SearchResponse {
        part: Some(part.into()),
        ..Default::default()
    }
}

/// `query`, unless it is blank, which a search refuses.
fn searched_query(query: &str) -> Result<&str, ConnectError> {
    if query.trim().is_empty() {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the words to search for must not be blank",
        )
        .field("query")
        .into());
    }
    Ok(query)
}

/// The one kind of title `kind` asks for, or `None` for every kind when it's unset.
fn searched_kind(kind: EnumValue<proto::TitleKind>) -> Result<Option<TitleKind>, ConnectError> {
    if kind == EnumValue::Known(proto::TitleKind::TITLE_KIND_UNSPECIFIED) {
        return Ok(None);
    }
    title_kind_from_proto(kind).map(Some)
}

/// A title a search of the Libraries found, which the caller can watch.
fn hit_to_proto(found: Found, images: &ShownImages) -> proto::Hit {
    let title = match found {
        Found::Movie(movie) => movie_hit(movie, images).into(),
        Found::Show(show) => show_hit(show, images).into(),
        Found::Episode(episode) => episode_hit(episode).into(),
    };
    proto::Hit {
        title,
        status: TitleStatus::TITLE_STATUS_AVAILABLE.into(),
        ..Default::default()
    }
}

/// Every id of each title of `kind` in `found`, which the title source's batch of that kind
/// leaves out. Kept apart by kind, since a title source may give a Movie and a Show one id.
fn library_ids(found: &[Found], kind: TitleKind) -> HashSet<ExternalId> {
    (found.iter())
        .filter_map(|found| match (found, kind) {
            (Found::Movie(movie), TitleKind::Movie) => {
                Some(movie.ids.iter().chain([movie.id.clone()]))
            }
            (Found::Show(show), TitleKind::Show) => Some(show.ids.iter().chain([show.id.clone()])),
            _ => None,
        })
        .flatten()
        .collect()
}

/// The first `limit` of what the title source's search of one kind `searched` found, ranked, as a
/// batch; `None` if it wasn't asked, or the caller may not request that kind.
fn batch<T>(
    searched: Option<Result<Option<Vec<T>>, requests::Error>>,
    limit: usize,
    to_hit: impl FnMut(T) -> proto::Hit,
) -> Result<Option<Hits>, requests::Error> {
    let found = searched.transpose()?.flatten();
    Ok(found.map(|found| Hits {
        hits: found.into_iter().take(limit).map(to_hit).collect(),
        ..Default::default()
    }))
}

/// What the stream sends after the Libraries' batch: each kind's `batches`, then `Done`, flagging
/// a kind the title source couldn't search. A kind that failed otherwise ends it with its error,
/// after the batches before it.
fn to_request_parts(
    batches: [Result<Option<Hits>, requests::Error>; 2],
) -> Vec<Result<SearchResponse, ConnectError>> {
    let mut parts = Vec::new();
    let mut done = Done::default();
    for searched in batches {
        match searched {
            Ok(hits) => parts.extend(hits.map(|hits| Ok(response(hits)))),
            Err(requests::Error::Unavailable) => done.title_source_unavailable = true,
            Err(error) => {
                parts.push(Err(error.into()));
                return parts;
            }
        }
    }
    parts.push(Ok(response(done)));
    parts
}

/// How close a title the title source found is to being on the Server.
fn status(library: Option<i64>, requested: bool) -> TitleStatus {
    match (library, requested) {
        (Some(_), _) => TitleStatus::TITLE_STATUS_AVAILABLE,
        (None, true) => TitleStatus::TITLE_STATUS_REQUESTED,
        (None, false) => TitleStatus::TITLE_STATUS_NOT_ON_THIS_SERVER,
    }
}

/// A Movie the title source found, with how close it is to being on the Server.
fn movie_to_request_hit(found: requests::Found<Candidate>, images: &ShownImages) -> proto::Hit {
    proto::Hit {
        status: status(found.library, found.request.is_some()).into(),
        title: searched_to_proto(found, images).into(),
        ..Default::default()
    }
}

/// A Show the title source found, with how close it is to being on the Server.
fn show_to_request_hit(found: ShowToRequest, images: &ShownImages) -> proto::Hit {
    let requested = found.request.is_some() || found.followed;
    let show = proto::ShowHit {
        library_id: found.library,
        request: found.request.into(),
        ..searched_show_to_proto(found.hit, images)
    };
    proto::Hit {
        status: status(found.library, requested).into(),
        title: show.into(),
        ..Default::default()
    }
}

fn movie_hit(movie: FoundMovie, images: &ShownImages) -> proto::MovieHit {
    let downloaded = movie.poster.as_ref().map(Downloaded::stored);
    let role = TitleImage::Movie(Role::Poster);
    let poster = images.of_title(&movie.id, &movie.ids, role, downloaded);
    proto::MovieHit {
        id: external_id_to_proto(&movie.id).into(),
        title: movie.title,
        year: movie.release_date.as_deref().and_then(year_of),
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        library_id: Some(movie.library),
        ..Default::default()
    }
}

fn show_hit(show: FoundShow, images: &ShownImages) -> proto::ShowHit {
    let downloaded = show.poster.as_ref().map(Downloaded::stored);
    let role = TitleImage::Show(Role::Poster);
    let poster = images.of_title(&show.id, &show.ids, role, downloaded);
    proto::ShowHit {
        id: external_id_to_proto(&show.id).into(),
        title: show.title,
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        library_id: Some(show.library),
        ..Default::default()
    }
}

fn episode_hit(episode: FoundEpisode) -> proto::EpisodeHit {
    proto::EpisodeHit {
        library_id: episode.library,
        id: external_id_to_proto(&episode.id).into(),
        show_id: external_id_to_proto(&episode.show).into(),
        show_title: episode.show_title,
        season_number: proto_number(episode.season),
        episode_number: proto_number(episode.number),
        title: episode.title,
        ..Default::default()
    }
}
