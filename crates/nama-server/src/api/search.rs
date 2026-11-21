//! Finding titles: `nama.v1.SearchService` (ADR 0080).

use buffa::EnumValue;
use connectrpc::{
    ConnectError, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::stream;
use nama_proto::nama::v1::{
    self as proto, Done, Hits, RefusalReason, SearchRequest, SearchResponse, SearchService,
    TitleStatus,
};

use crate::{
    browse::Browse,
    library::{
        Downloaded, Found, FoundEpisode, FoundMovie, FoundShow, ImageSources, Role, ShownImages,
        TitleImage, TitleKind,
    },
    requests::year_of,
};

use super::{database_failed, external_id_to_proto, proto_number, refuse, title_kind_from_proto};

/// How many results a batch holds when the request doesn't say.
const DEFAULT_LIMIT: usize = 20;

/// The most results a batch holds, whatever the request says.
const MAX_LIMIT: usize = 100;

/// `nama.v1.SearchService`: finding titles, for any signed-in User.
pub(super) struct SearchServiceHandler {
    pub(super) browse: Browse,
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
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchRequest>,
    ) -> ServiceResult<ServiceStream<SearchResponse>> {
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
        let hits = Hits {
            hits: (found.into_iter())
                .map(|found| hit_to_proto(found, &images))
                .collect(),
            ..Default::default()
        };
        let parts = [hits.into(), Done::default().into()].map(|part| {
            Ok(SearchResponse {
                part,
                ..Default::default()
            })
        });
        let stream: ServiceStream<SearchResponse> = Box::pin(stream::iter(parts));
        Response::ok(stream)
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
