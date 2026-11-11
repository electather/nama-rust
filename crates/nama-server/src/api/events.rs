//! Live updates: `nama.v1.EventService`.

use std::future;

use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::StreamExt;
use nama_proto::nama::v1::{EventService, EventType, SubscribeRequest, SubscribeResponse};

use crate::{
    events::{EventBus, Filter},
    users::Users,
};

use super::{caller, unknown_enum_value};

/// `nama.v1.EventService`: live updates.
//
// TODO(M3): once there is per-Library access, only send a caller events about Libraries it may see.
pub(super) struct EventServiceHandler {
    pub(super) events: EventBus,
    /// Whose sign-outs end the streams their Devices opened.
    pub(super) users: Users,
}

impl EventService for EventServiceHandler {
    fn subscribe(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SubscribeRequest>,
    ) -> impl Future<
        Output = ServiceResult<ServiceStream<impl Encodable<SubscribeResponse> + Send + use<>>>,
    > + Send {
        let request = request.to_owned_message();
        let subscribed = caller(&ctx).and_then(|caller| {
            let types = request
                .types
                .into_iter()
                .map(event_type_from_proto)
                .collect::<Result<_, _>>()?;
            let filter = Filter {
                types,
                libraries: request.library_ids,
            };
            let signed_out = self.users.signed_out(caller.device.id);
            let events = self
                .events
                .subscribe(&caller.user, filter)
                .take_until(signed_out);
            let stream: ServiceStream<SubscribeResponse> = Box::pin(events.map(Ok));
            Response::ok(stream)
        });
        future::ready(subscribed)
    }
}

fn event_type_from_proto(kind: EnumValue<EventType>) -> Result<EventType, ConnectError> {
    match kind {
        EnumValue::Known(EventType::EVENT_TYPE_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            unknown_enum_value("types", "an event type must be one this Server knows"),
        ),
        EnumValue::Known(kind) => Ok(kind),
    }
}
