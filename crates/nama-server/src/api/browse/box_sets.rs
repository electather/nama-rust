//! Box Sets as clients see them: `GetBoxSet`, `ListBoxSets` and a Movie's own.

use connectrpc::{RequestContext, Response, ServiceResult};
use nama_proto::nama::v1::{
    self as proto, GetBoxSetRequest, GetBoxSetResponse, ListBoxSetsRequest, ListBoxSetsResponse,
};

use super::{BrowseServiceHandler, Fields, refused, row_title_to_proto};
use crate::{
    api::{caller, database_failed, external_id_to_proto, required_id},
    browse::SeenBoxSet,
    library::{Downloaded, ShownImages, StoredBoxSet},
};

pub(super) async fn get_box_set(
    handler: &BrowseServiceHandler,
    ctx: &RequestContext,
    request: GetBoxSetRequest,
) -> ServiceResult<GetBoxSetResponse> {
    let user = &caller(ctx)?.user;
    let id = required_id(request.box_set_id.into_option(), "box_set_id", "Box Set")?;
    let seen = handler.pages.box_set(user, &id).await;
    let seen = seen.map_err(refused(Fields {
        title: "box_set_id",
        ..Fields::NONE
    }))?;
    let images = handler.images.shown().await.map_err(database_failed)?;
    Response::ok(GetBoxSetResponse {
        box_set: box_set_to_proto(seen, &images).into(),
        ..Default::default()
    })
}

pub(super) async fn list_box_sets(
    handler: &BrowseServiceHandler,
    ctx: &RequestContext,
    request: ListBoxSetsRequest,
) -> ServiceResult<ListBoxSetsResponse> {
    let user = &caller(ctx)?.user;
    let listed = (handler.pages)
        .box_sets(user, request.library_id, &request.page)
        .await;
    let (box_sets, page) = listed.map_err(refused(Fields::LIBRARY))?;
    let images = handler.images.shown().await.map_err(database_failed)?;
    Response::ok(ListBoxSetsResponse {
        box_sets: (box_sets.into_iter())
            .map(|box_set| box_set_summary(box_set, &images))
            .collect(),
        page: page.into(),
        ..Default::default()
    })
}

/// A Box Set as a User sees it, its artwork offered as a Home row's missing titles' is, since no
/// Match stores it.
pub(super) fn box_set_to_proto(seen: SeenBoxSet, images: &ShownImages) -> proto::BoxSet {
    let SeenBoxSet { box_set, movies } = seen;
    let backdrop = images.downloaded(box_set.backdrop.as_ref().map(Downloaded::offered));
    let overview = box_set.overview.clone();
    let summary = box_set_summary(box_set, images);
    proto::BoxSet {
        id: summary.id,
        name: summary.name,
        overview,
        poster_url: summary.poster_url,
        backdrop_url: backdrop.url,
        movies: (movies.into_iter())
            .map(|title| row_title_to_proto(title, images))
            .collect(),
        ..Default::default()
    }
}

fn box_set_summary(box_set: StoredBoxSet, images: &ShownImages) -> proto::BoxSetSummary {
    let poster = images.downloaded(box_set.poster.as_ref().map(Downloaded::offered));
    proto::BoxSetSummary {
        id: external_id_to_proto(&box_set.id).into(),
        name: box_set.name,
        poster_url: poster.url,
        ..Default::default()
    }
}
