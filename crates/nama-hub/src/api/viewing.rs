//! Viewing Events and pooling: `nama.hub.v1.ViewingEventService` for a signed-in Account, and the
//! conversions `LinkedServerService.RecordViewingEvents` uses.

use buffa::EnumValue;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_hub_proto::nama::hub::v1::{
    self as proto, DeleteViewingEventRequest, DeleteViewingEventResponse,
    DeviceKind as ProtoDevice, GetPoolingRequest, GetPoolingResponse, ListViewingEventsRequest,
    ListViewingEventsResponse, RefusalReason, SetPoolingRequest, SetPoolingResponse,
    ViewingEventService,
};

use super::{accounts::signed_in, hub_failed, refuse};
use crate::viewing::{
    self, DeviceKind, Kind, ListedViewingEvent, MAX_RECORDED, UserViewingEvent, Viewing,
    ViewingEvent, ViewingEventId,
};

/// `nama.hub.v1.ViewingEventService`: a signed-in Account.
pub(super) struct ViewingEventServiceHandler {
    pub(super) viewing: Viewing,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl ViewingEventService for ViewingEventServiceHandler {
    async fn list_viewing_events(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, ListViewingEventsRequest>,
    ) -> ServiceResult<ListViewingEventsResponse> {
        let account = signed_in(&ctx)?.account.id;
        let events = (self.viewing.list(account))
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(ListViewingEventsResponse {
            events: events.into_iter().map(event_to_proto).collect(),
            ..Default::default()
        })
    }

    async fn delete_viewing_event(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, DeleteViewingEventRequest>,
    ) -> ServiceResult<DeleteViewingEventResponse> {
        let account = signed_in(&ctx)?.account.id;
        // An id that isn't a UUID names no Viewing Event, so deleting it does nothing.
        if let Ok(id) = request.id.parse::<ViewingEventId>() {
            (self.viewing.delete(account, id))
                .await
                .map_err(|err| hub_failed(&err))?;
        }
        Response::ok(DeleteViewingEventResponse::default())
    }

    async fn get_pooling(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetPoolingRequest>,
    ) -> ServiceResult<GetPoolingResponse> {
        let account = signed_in(&ctx)?.account.id;
        let pooling = (self.viewing.pooling(account))
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(GetPoolingResponse {
            pooling,
            ..Default::default()
        })
    }

    async fn set_pooling(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetPoolingRequest>,
    ) -> ServiceResult<SetPoolingResponse> {
        let account = signed_in(&ctx)?.account.id;
        (self.viewing.set_pooling(account, request.pooling))
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(SetPoolingResponse::default())
    }
}

/// The Viewing Events a Linked Server sent, or the refusal for one it can't have meant.
pub(super) fn recorded_from_proto(
    events: Vec<proto::UserViewingEvent>,
) -> Result<Vec<UserViewingEvent>, ConnectError> {
    events
        .into_iter()
        .map(|recorded| {
            let event = recorded.event.into_option().unwrap_or_default();
            let kind = if event.episode {
                Kind::Episode
            } else {
                Kind::Movie
            };
            let device_kind = match event.device_kind {
                EnumValue::Known(ProtoDevice::DEVICE_KIND_COMPUTER) => DeviceKind::Computer,
                EnumValue::Known(ProtoDevice::DEVICE_KIND_PHONE) => DeviceKind::Phone,
                EnumValue::Known(ProtoDevice::DEVICE_KIND_TABLET) => DeviceKind::Tablet,
                EnumValue::Known(ProtoDevice::DEVICE_KIND_TV) => DeviceKind::Tv,
                EnumValue::Known(ProtoDevice::DEVICE_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => {
                    DeviceKind::Unknown
                }
            };
            Ok(UserViewingEvent {
                user_id: recorded.user_id,
                key: recorded.key,
                event: ViewingEvent {
                    kind,
                    external_id: event.external_id,
                    started_at: event.started_at,
                    time_of_day_minutes: event.time_of_day_minutes,
                    stop_position_ms: event.stop_position_ms,
                    duration_ms: event.duration_ms,
                    finished: event.finished,
                    device_kind,
                    show_external_id: Some(event.show_external_id).filter(|id| !id.is_empty()),
                },
            })
        })
        .collect()
}

fn event_to_proto(listed: ListedViewingEvent) -> proto::ViewingEvent {
    let ListedViewingEvent { id, event } = listed;
    proto::ViewingEvent {
        id: id.to_string(),
        episode: event.kind == Kind::Episode,
        external_id: event.external_id,
        started_at: event.started_at,
        time_of_day_minutes: event.time_of_day_minutes,
        stop_position_ms: event.stop_position_ms,
        duration_ms: event.duration_ms,
        finished: event.finished,
        device_kind: match event.device_kind {
            DeviceKind::Unknown => ProtoDevice::DEVICE_KIND_UNSPECIFIED,
            DeviceKind::Computer => ProtoDevice::DEVICE_KIND_COMPUTER,
            DeviceKind::Phone => ProtoDevice::DEVICE_KIND_PHONE,
            DeviceKind::Tablet => ProtoDevice::DEVICE_KIND_TABLET,
            DeviceKind::Tv => ProtoDevice::DEVICE_KIND_TV,
        }
        .into(),
        show_external_id: event.show_external_id.unwrap_or_default(),
        ..Default::default()
    }
}

/// The Connect error a Linked Server gets for Viewing Events that couldn't be recorded.
pub(super) fn refused(err: &viewing::Error) -> ConnectError {
    match err {
        viewing::Error::TooMany | viewing::Error::Invalid => refuse(
            RefusalReason::REFUSAL_REASON_VIEWING_EVENT_INVALID,
            err.to_string(),
        )
        .field("events")
        .param("maxEvents", &MAX_RECORDED)
        .into(),
        viewing::Error::Database(_) => hub_failed(err),
    }
}
