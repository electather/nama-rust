//! Fixing Matches by hand: `nama.v1.MatchService`.

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, ClearMatchRequest, ClearMatchResponse, ListUnmatchedItemsRequest,
    ListUnmatchedItemsResponse, MatchService, RefusalReason, SearchTitlesRequest,
    SearchTitlesResponse, SetMatchRequest, SetMatchResponse, SortDirection, UnmatchedItemSort,
};

use crate::{
    library::{
        Contents, Libraries, SearchQuery, TiedTitle, TitleSources, Unanswered, Unmatched,
        UnmatchedReason,
    },
    matching::{ManualError, Matcher},
    page::Page,
};

use super::{
    browse::unmatched_summary, database_failed, external_id_to_proto, refuse, required_id,
    title_kind_from_proto,
};

/// `nama.v1.MatchService`: fixing Matches by hand. Needs `PERMISSION_MANAGE_LIBRARIES`, which
/// [`require_access`](super::require_access) enforces.
pub(super) struct MatchServiceHandler {
    pub(super) matcher: Matcher,
    pub(super) metadata: TitleSources,
    pub(super) contents: Contents,
    pub(super) libraries: Libraries,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl MatchService for MatchServiceHandler {
    async fn search_titles(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, SearchTitlesRequest>,
    ) -> ServiceResult<SearchTitlesResponse> {
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
        // A Match search belongs to no Library, so it asks in Admin precedence.
        let candidates = match self.metadata.search_in_turn(&query).await {
            Ok((_, candidates)) => candidates,
            Err(Unanswered::NotFound) => Vec::new(),
            Err(Unanswered::Unavailable) => {
                return Err(ManualError::Unavailable.into());
            }
        };
        Response::ok(SearchTitlesResponse {
            candidates: candidates
                .into_iter()
                .map(|candidate| title_to_proto(candidate.into()))
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
        let title = required_id(request.title_id.into_option(), "title_id", "title")?;
        let summary = self.matcher.set_match(request.item_id, &title).await?;
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
        let summary = self.matcher.clear_match(request.item_id).await?;
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
        let request = request.to_owned_message();
        let library = request.filter.library_id;
        let searching = self
            .metadata
            .in_order(self.libraries.get(library).await?.title_sources)
            .can_search()
            .await;
        let order = unmatched_order(&request);
        let list = format!("ListUnmatchedItems {library} {order:?}");
        let page = Page::read(&request.page, list.as_bytes()).map_err(|invalid| {
            ConnectError::from(
                refuse(
                    RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
                    invalid.to_string(),
                )
                .field("page.token"),
            )
        })?;
        let (unmatched, paging) = (self.contents)
            .unmatched(library, order, &page)
            .await
            .map_err(database_failed)?;
        let unparsed_paths = if page.is_first() {
            (self.contents.unparsed(library).await).map_err(database_failed)?
        } else {
            Vec::new()
        };
        Response::ok(ListUnmatchedItemsResponse {
            items: (unmatched.into_iter())
                .map(|item| unmatched_to_proto(item, searching))
                .collect(),
            unparsed_paths,
            page: paging.into(),
            ..Default::default()
        })
    }
}

/// The order `request` asks for, with each sort's natural direction when it names none: by path
/// from A, and the Item found most recently first.
fn unmatched_order(request: &ListUnmatchedItemsRequest) -> (UnmatchedItemSort, SortDirection) {
    use {SortDirection as D, UnmatchedItemSort as S};
    let sort = match request.sort.as_known() {
        Some(S::UNMATCHED_ITEM_SORT_FOUND) => S::UNMATCHED_ITEM_SORT_FOUND,
        _ => S::UNMATCHED_ITEM_SORT_PATH,
    };
    let direction = match request.direction.as_known() {
        Some(direction @ (D::SORT_DIRECTION_ASCENDING | D::SORT_DIRECTION_DESCENDING)) => direction,
        _ if sort == S::UNMATCHED_ITEM_SORT_FOUND => D::SORT_DIRECTION_DESCENDING,
        _ => D::SORT_DIRECTION_ASCENDING,
    };
    (sort, direction)
}

fn title_to_proto(title: TiedTitle) -> proto::TitleCandidate {
    proto::TitleCandidate {
        id: external_id_to_proto(&title.id).into(),
        title: title.title,
        original_title: title.original_title,
        year: title.year,
        ..Default::default()
    }
}

fn reason_to_proto(
    reason: Option<UnmatchedReason>,
) -> (proto::UnmatchedReason, Vec<proto::TitleCandidate>) {
    use proto::UnmatchedReason as R;
    match reason {
        None => (R::UNMATCHED_REASON_UNSPECIFIED, Vec::new()),
        Some(UnmatchedReason::Ambiguous(tied)) => (
            R::UNMATCHED_REASON_AMBIGUOUS,
            tied.into_iter().map(title_to_proto).collect(),
        ),
        Some(UnmatchedReason::NoTitleFits) => (R::UNMATCHED_REASON_NO_TITLE_FITS, Vec::new()),
        Some(UnmatchedReason::UnknownIdHint) => (R::UNMATCHED_REASON_UNKNOWN_ID_HINT, Vec::new()),
        Some(UnmatchedReason::NotInShow) => (R::UNMATCHED_REASON_NOT_IN_SHOW, Vec::new()),
    }
}

/// `unmatched` as listed, where `searching` says whether some source in its Library's order
/// searches, without which Matching can't decide it.
fn unmatched_to_proto(unmatched: Unmatched, searching: bool) -> proto::UnmatchedItem {
    let (reason, tied_titles) = if unmatched.cleared {
        (
            proto::UnmatchedReason::UNMATCHED_REASON_CLEARED_BY_HAND,
            Vec::new(),
        )
    } else if unmatched.undecided && !searching {
        (
            proto::UnmatchedReason::UNMATCHED_REASON_NO_SEARCHING_SOURCE,
            Vec::new(),
        )
    } else {
        reason_to_proto(unmatched.reason)
    };
    let proto::UnmatchedItemSummary {
        version_id,
        kind,
        title,
        year,
        season,
        episode,
        ..
    } = unmatched_summary(unmatched.item, unmatched.version_id);
    proto::UnmatchedItem {
        item_id: unmatched.id,
        version_id,
        path: unmatched.path,
        kind,
        title,
        year,
        season,
        episode,
        reason: reason.into(),
        tied_titles,
        ..Default::default()
    }
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
            ManualError::Unavailable => refuse(R::REFUSAL_REASON_NO_TITLE_SOURCE, message),
            ManualError::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}
