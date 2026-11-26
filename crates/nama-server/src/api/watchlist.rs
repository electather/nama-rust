//! Each signed-in User's Watchlist: `nama.v1.WatchlistService`. Each change is a trigger for
//! `Syncing` (ADR 0056). A Show on it is a Followed Show of a User who may Request Shows.

use std::sync::Arc;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_plugin_api::metadata::ExternalId;
use nama_proto::nama::v1::{
    self as proto, AddToWatchlistRequest, AddToWatchlistResponse, ListWatchlistRequest,
    ListWatchlistResponse, Permission, RemoveFromWatchlistRequest, RemoveFromWatchlistResponse,
    WatchlistService,
};

use crate::{
    db::Database,
    home_rows::HomeRows,
    library::{ImageSources, TitleKind},
    requests::{self, Requests},
    sync::Syncing,
    watchlist,
};

use super::{
    browse::{home_rows_refused, row_title_to_proto},
    caller, database_failed, external_id_to_proto, required_id, shown_images,
    title_kind_from_proto,
};

/// `nama.v1.WatchlistService`: each signed-in User's Watchlist.
pub(super) struct WatchlistServiceHandler {
    pub(super) db: Arc<Database>,
    /// Follows each Show added, and stops when it's taken off.
    pub(super) requests: Requests,
    /// Told of each change.
    pub(super) syncing: Syncing,
    /// Shows each listed title as a Home row would.
    pub(super) home_rows: HomeRows,
    pub(super) images: ImageSources,
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
        let caller = &caller(&ctx)?.user;
        let user = caller.id;
        let AddToWatchlistRequest { kind, id, .. } = request.to_owned_message();
        let (kind, id) = title(kind, id.into_option())?;
        // Followed first, so a title source that's down fails the add and the User can try again.
        // Through Hub sync a Show is added without following it.
        if kind == TitleKind::Show && caller.permissions.allow(Permission::RequestTv) {
            match self.requests.follow_new_seasons(caller, &id).await {
                // A Show only a Library knows, by an id no title source looks up, has no Seasons
                // to follow.
                Ok(_) | Err(requests::Error::NoSuchShow(_)) => {}
                Err(err) => return Err(err.into()),
            }
        }
        let added = watchlist::add(&self.db, user, kind, &id).await;
        added.map_err(database_failed)?;
        self.syncing.watchlist_changed(user);
        Response::ok(AddToWatchlistResponse::default())
    }

    async fn remove_from_watchlist(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, RemoveFromWatchlistRequest>,
    ) -> ServiceResult<RemoveFromWatchlistResponse> {
        let caller = &caller(&ctx)?.user;
        let user = caller.id;
        let RemoveFromWatchlistRequest { kind, id, .. } = request.to_owned_message();
        let (kind, id) = title(kind, id.into_option())?;
        if kind == TitleKind::Show {
            // Whatever their permissions now: they may have followed it before losing REQUEST_TV.
            self.requests.stop_following(caller, &id).await?;
        }
        let removed = watchlist::remove(&self.db, user, kind, &id).await;
        removed.map_err(database_failed)?;
        self.syncing.watchlist_changed(user);
        Response::ok(RemoveFromWatchlistResponse::default())
    }

    async fn list_watchlist(
        &self,
        ctx: RequestContext,
        _: ServiceRequest<'_, ListWatchlistRequest>,
    ) -> ServiceResult<ListWatchlistResponse> {
        let reader = &caller(&ctx)?.user;
        let listed = watchlist::list(&self.db, reader.id).await;
        let shown = self
            .home_rows
            .shown_to(reader, listed.map_err(database_failed)?)
            .await;
        let shown = shown.map_err(home_rows_refused)?;
        let images = shown_images(&self.images, &ctx).await?;
        Response::ok(ListWatchlistResponse {
            titles: (shown.into_iter())
                .map(|(id, title)| proto::WatchlistTitle {
                    id: external_id_to_proto(&id).into(),
                    title: row_title_to_proto(title, &images).into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
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
