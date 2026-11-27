//! Live updates: `nama.v1.EventService`.

use std::sync::Arc;

use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::StreamExt;
use nama_proto::nama::v1::{EventService, EventType, SubscribeRequest, SubscribeResponse};

use crate::{
    events::{EventBus, Filter, MatchRatings},
    library::Contents,
    users::Users,
};

use super::{caller, database_failed, unknown_enum_value};

/// `nama.v1.EventService`: live updates.
pub(super) struct EventServiceHandler {
    pub(super) events: EventBus,
    /// Whose sign-outs end the streams their Devices opened, and whose Permission and Reach
    /// changes end the streams they opened, so a stream never sees what its User no longer may.
    pub(super) users: Users,
    /// Where the Content Rating of a Matched title is read, for a capped subscriber's Maximum Age.
    pub(super) contents: Contents,
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
        let caller = caller(&ctx).cloned();
        let (events, users) = (self.events.clone(), self.users.clone());
        let matches = match_ratings(self.contents.clone());
        async move {
            let caller = caller?;
            let types = request
                .types
                .into_iter()
                .map(event_type_from_proto)
                .collect::<Result<_, _>>()?;
            let filter = Filter {
                types,
                libraries: request.library_ids,
            };
            let signed_out = users.signed_out(caller.device.id, caller.user.id);
            let permissions_changed =
                users.permissions_changed(caller.user.id, caller.user.permissions);
            let watched = users.reach_watched(caller.user.id).await;
            let (reach, reach_changed) = watched.map_err(database_failed)?;
            let events = events
                .subscribe(&caller.user, reach, matches, filter)
                .take_until(signed_out)
                .take_until(permissions_changed)
                .take_until(reach_changed);
            let stream: ServiceStream<SubscribeResponse> = Box::pin(events.map(Ok));
            Response::ok(stream)
        }
    }
}

/// Each Matched title's Content Rating, as `contents` holds it.
fn match_ratings(contents: Contents) -> MatchRatings {
    Arc::new(move |item, title| {
        let contents = contents.clone();
        Box::pin(async move { contents.item_content_rating(item, &title).await })
    })
}

fn event_type_from_proto(kind: EnumValue<EventType>) -> Result<EventType, ConnectError> {
    match kind {
        EnumValue::Known(EventType::EVENT_TYPE_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            unknown_enum_value("types", "an event type must be one this Server knows"),
        ),
        EnumValue::Known(kind) => Ok(kind),
    }
}
