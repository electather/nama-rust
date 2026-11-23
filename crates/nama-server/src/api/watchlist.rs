//! Each signed-in User's Watchlist: `nama.v1.WatchlistService`. Each change syncs it with the
//! enabled Plugin that links the Server (ADR 0056).

use std::sync::Arc;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::{
    self as proto, AddToWatchlistRequest, AddToWatchlistResponse, RemoveFromWatchlistRequest,
    RemoveFromWatchlistResponse, WatchlistService,
};

use crate::{db::Database, library::TitleKind, plugins::ExtensionPoints, watchlist};

use super::{caller, database_failed, required_id, server_link, title_kind_from_proto};

/// `nama.v1.WatchlistService`: each signed-in User's Watchlist.
pub(super) struct WatchlistServiceHandler {
    pub(super) db: Arc<Database>,
    /// Syncs a changed Watchlist.
    pub(super) points: ExtensionPoints,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl WatchlistService for WatchlistServiceHandler {
    async fn add_to_watchlist(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, AddToWatchlistRequest>,
    ) -> ServiceResult<AddToWatchlistResponse> {
        let user = caller(&ctx)?.user.id;
        let AddToWatchlistRequest { kind, id, .. } = request.to_owned_message();
        let (kind, id) = title(kind, id.into_option())?;
        let added = watchlist::add(&self.db, user, kind, &id).await;
        added.map_err(database_failed)?;
        server_link::sync_watchlist_soon(&self.points, &self.db, user);
        Response::ok(AddToWatchlistResponse::default())
    }

    async fn remove_from_watchlist(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RemoveFromWatchlistRequest>,
    ) -> ServiceResult<RemoveFromWatchlistResponse> {
        let user = caller(&ctx)?.user.id;
        let RemoveFromWatchlistRequest { kind, id, .. } = request.to_owned_message();
        let (kind, id) = title(kind, id.into_option())?;
        let removed = watchlist::remove(&self.db, user, kind, &id).await;
        removed.map_err(database_failed)?;
        server_link::sync_watchlist_soon(&self.points, &self.db, user);
        Response::ok(RemoveFromWatchlistResponse::default())
    }
}

/// The title a request names by its `kind` and `id`, both required.
fn title(
    kind: buffa::EnumValue<proto::TitleKind>,
    id: Option<proto::ExternalId>,
) -> Result<(TitleKind, ExternalId), ConnectError> {
    Ok((
        title_kind_from_proto(kind)?,
        required_id(id, "id", "title")?,
    ))
}
