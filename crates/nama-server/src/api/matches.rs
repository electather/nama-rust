//! Fixing Matches by hand: `nama.v1.MatchService`.

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::{PluginId, metadata::ExternalId};
use nama_proto::nama::v1::{
    self as proto, ClearMatchRequest, ClearMatchResponse, ListUnmatchedItemsRequest,
    ListUnmatchedItemsResponse, MatchService, RefusalReason, SearchTitlesRequest,
    SearchTitlesResponse, SetMatchRequest, SetMatchResponse, SortDirection, UnmatchedItemSort,
};

use crate::{
    library::{
        Artwork, Candidate, Contents, Libraries, SearchQuery, TiedTitle, TitleSources, Unanswered,
        Unmatched, UnmatchedReason,
    },
    matching::{ManualError, Matcher},
    page::Page,
    signed_urls::{UrlSigner, signed_by},
};

use super::{
    browse::{Fields, not_found, unmatched_summary},
    database_failed, external_id_to_proto, refuse, required_id, title_kind_from_proto, url_signer,
};

/// `nama.v1.MatchService`: fixing Matches by hand. Needs `PERMISSION_MANAGE_LIBRARIES`, which
/// [`require_access`](super::require_access) enforces.
pub(super) struct MatchServiceHandler {
    pub(super) matcher: Matcher,
    pub(super) metadata: TitleSources,
    pub(super) contents: Contents,
    pub(super) libraries: Libraries,
    /// Offers the Posters of the titles a search finds.
    pub(super) artwork: Artwork,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl MatchService for MatchServiceHandler {
    async fn search_titles(
        &self,
        ctx: RequestContext,
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
        // A Match search naming no Library asks in Admin precedence.
        let sources = match request.library_id {
            Some(library) => self.title_sources_of(library).await?,
            None => self.metadata.clone(),
        };
        let candidates = match sources.search_in_turn(&query).await {
            Ok((source, candidates)) => (candidates.into_iter())
                .map(|candidate| {
                    self.candidate_to_proto(&source.plugin, candidate, url_signer(&ctx))
                })
                .collect(),
            Err(Unanswered::NotFound) => Vec::new(),
            Err(Unanswered::Unavailable) => {
                return Err(ManualError::Unavailable.into());
            }
        };
        Response::ok(SearchTitlesResponse {
            candidates,
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
        let summary = match request.matched_title.into_option() {
            Some(matched) => {
                let (library, matched) = matched_title_from_proto(matched)?;
                self.matcher
                    .set_title_match(library, &matched, &title)
                    .await?
            }
            None => self.matcher.set_match(request.item_id, &title).await?,
        };
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
        let request = request.to_owned_message();
        let summary = match request.matched_title.into_option() {
            Some(matched) => {
                let (library, matched) = matched_title_from_proto(matched)?;
                self.matcher.clear_title_match(library, &matched).await?
            }
            None => self.matcher.clear_match(request.item_id).await?,
        };
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
        let searching = self.title_sources_of(library).await?.can_search().await;
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

impl MatchServiceHandler {
    /// The title sources of the Library `library`, in its order.
    async fn title_sources_of(&self, library: i64) -> Result<TitleSources, ConnectError> {
        Ok((self.metadata).in_order(self.libraries.get(library).await?.title_sources))
    }

    /// `candidate`, which the Plugin `plugin`'s title source found, with its Poster offered and
    /// signed by `signer` if there is one.
    fn candidate_to_proto(
        &self,
        plugin: &PluginId,
        candidate: Candidate,
        signer: Option<&UrlSigner>,
    ) -> proto::TitleCandidate {
        let poster_url = (candidate.poster.as_deref())
            .map(|path| signed_by(signer, self.artwork.offer(plugin, path)));
        proto::TitleCandidate {
            poster_url,
            ..title_to_proto(candidate.into())
        }
    }
}

/// The Library and the id of the Movie or Show `matched` names.
fn matched_title_from_proto(
    matched: proto::MatchedTitle,
) -> Result<(i64, ExternalId), ConnectError> {
    let id = required_id(
        matched.id.into_option(),
        "matched_title.id",
        "Movie or Show",
    )?;
    Ok((matched.library_id, id))
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
fn unmatched_to_proto(mut unmatched: Unmatched, searching: bool) -> proto::UnmatchedItem {
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
        reason_to_proto(unmatched.reason.take())
    };
    let path = std::mem::take(&mut unmatched.path);
    let proto::UnmatchedItemSummary {
        item_id,
        version_id,
        kind,
        title,
        year,
        season,
        episode,
        ..
    } = unmatched_summary(unmatched);
    proto::UnmatchedItem {
        item_id,
        version_id,
        path,
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

/// The fields of a request's `matched_title`.
const MATCHED_TITLE: Fields = Fields {
    library: "matched_title.library_id",
    title: "matched_title.id",
    ..Fields::NONE
};

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
            ManualError::NotFound(err) => not_found(MATCHED_TITLE, &err),
            ManualError::Database(err) => return database_failed(err),
        };
        refused.into()
    }
}
