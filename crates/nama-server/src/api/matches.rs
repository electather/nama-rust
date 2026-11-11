//! Fixing Matches by hand: `nama.v1.MatchService`.

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::metadata::{Candidate, SearchQuery};
use nama_proto::nama::v1::{
    self as proto, ClearMatchRequest, ClearMatchResponse, ListUnmatchedItemsRequest,
    ListUnmatchedItemsResponse, MatchService, RefusalReason, SearchMetadataRequest,
    SearchMetadataResponse, SetMatchRequest, SetMatchResponse,
};

use crate::{
    contents::{Contents, Item, Unmatched},
    libraries::Libraries,
    matching::{ManualError, Matcher},
    scanner::Scanner,
};

use super::{
    database_failed, external_id_from_proto, external_id_to_proto, refuse, title_kind_from_proto,
};

/// `nama.v1.MatchService`: fixing Matches by hand. Needs `PERMISSION_MANAGE_LIBRARIES`, which
/// [`require_access`](super::require_access) enforces.
pub(super) struct MatchServiceHandler {
    pub(super) matcher: Matcher,
    pub(super) contents: Contents,
    pub(super) libraries: Libraries,
    /// Where the scan lock a Match is fixed under comes from.
    pub(super) scanner: Scanner,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl MatchService for MatchServiceHandler {
    async fn search_metadata(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchMetadataRequest>,
    ) -> ServiceResult<SearchMetadataResponse> {
        let request = request.to_owned_message();
        if request.title.trim().is_empty() {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "the title to search for must not be blank",
            )
            .field("title")
            .into());
        }
        let query = SearchQuery {
            kind: title_kind_from_proto(request.kind)?,
            title: request.title,
            year: request.year,
        };
        let candidates = self.matcher.search_metadata(&query).await?;
        Response::ok(SearchMetadataResponse {
            candidates: candidates
                .into_iter()
                .map(|(_, candidate)| candidate_to_proto(candidate))
                .collect(),
            ..Default::default()
        })
    }

    async fn set_match(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SetMatchRequest>,
    ) -> ServiceResult<SetMatchResponse> {
        let request = request.to_owned_message();
        let title = request
            .title_id
            .into_option()
            .and_then(|id| id.id)
            .map(external_id_from_proto)
            .ok_or_else(|| {
                refuse(
                    RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                    "the title to Match to is missing",
                )
                .field("title_id")
            })?;
        let scan = self.scanner.lock_library_of(request.item_id).await?;
        let summary = self
            .matcher
            .set_match(&scan, request.item_id, &title)
            .await?;
        Response::ok(SetMatchResponse {
            matched: summary.matched,
            unmatched: summary.unmatched,
            ..Default::default()
        })
    }

    async fn clear_match(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ClearMatchRequest>,
    ) -> ServiceResult<ClearMatchResponse> {
        let scan = self.scanner.lock_library_of(request.item_id).await?;
        let summary = self.matcher.clear_match(&scan, request.item_id).await?;
        Response::ok(ClearMatchResponse {
            unmatched: summary.unmatched,
            ..Default::default()
        })
    }

    async fn list_unmatched_items(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, ListUnmatchedItemsRequest>,
    ) -> ServiceResult<ListUnmatchedItemsResponse> {
        self.libraries.get(request.library_id).await?;
        let unmatched = self
            .contents
            .unmatched(request.library_id)
            .await
            .map_err(database_failed)?;
        let unparsed_paths = self
            .contents
            .unparsed(request.library_id)
            .await
            .map_err(database_failed)?;
        Response::ok(ListUnmatchedItemsResponse {
            items: unmatched.into_iter().map(unmatched_to_proto).collect(),
            unparsed_paths,
            ..Default::default()
        })
    }
}

fn candidate_to_proto(candidate: Candidate) -> proto::TitleCandidate {
    proto::TitleCandidate {
        id: external_id_to_proto(candidate.id).into(),
        title: candidate.title,
        original_title: candidate.original_title,
        year: candidate.year,
        ..Default::default()
    }
}

fn unmatched_to_proto(unmatched: Unmatched) -> proto::UnmatchedItem {
    let (kind, title, year, numbers) = match unmatched.item {
        Item::Movie { title, year, .. } => (proto::TitleKind::TITLE_KIND_MOVIE, title, year, None),
        Item::Episode {
            show,
            year,
            season,
            episode,
            ..
        } => (
            proto::TitleKind::TITLE_KIND_SHOW,
            show,
            year,
            Some((season, episode)),
        ),
    };
    proto::UnmatchedItem {
        item_id: unmatched.id,
        path: unmatched.path,
        kind: kind.into(),
        title,
        year: year.and_then(int32),
        season: numbers.and_then(|(season, _)| int32(season)),
        episode: numbers.and_then(|(_, episode)| int32(episode)),
        ..Default::default()
    }
}

/// `n` as a proto `int32`, or `None` if it doesn't fit, which no name a scan parses gives.
fn int32(n: i64) -> Option<i32> {
    i32::try_from(n).ok()
}

/// The Connect error a client gets for fixing a Match that failed. Database failures are logged,
/// not sent: they say nothing the client can act on.
impl From<ManualError> for ConnectError {
    fn from(err: ManualError) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            // Only `SetMatch` and `ClearMatch` name an item.
            ManualError::NoSuchItem(id) => refuse(R::REFUSAL_REASON_ITEM_NOT_FOUND, message)
                .field("item_id")
                .param("itemId", &id),
            ManualError::NoSuchTitle(id) => refuse(R::REFUSAL_REASON_UNKNOWN_TITLE, message)
                .field("title_id")
                .param("titleId", &id),
            ManualError::Unavailable => refuse(R::REFUSAL_REASON_NO_METADATA_PROVIDER, message),
            ManualError::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}
