//! Live updates: `nama.v1.EventService`.

use std::sync::Arc;

use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::StreamExt;
use nama_proto::nama::v1::{EventService, EventType, SubscribeRequest, SubscribeResponse};

use crate::{
    content_ratings::MaximumAge,
    events::{AgeCheck, EventBus, Filter},
    library::Contents,
    users::Users,
};

use super::{caller, database_failed, unknown_enum_value};

/// `nama.v1.EventService`: live updates.
pub(super) struct EventServiceHandler {
    pub(super) events: EventBus,
    /// Whose sign-outs end the streams their Devices opened, and whose Permission, Library access
    /// and Maximum Age changes end the streams they opened, so a stream never sees what its User
    /// no longer may.
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
        let contents = self.contents.clone();
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
            let watched = users.library_access_watched(caller.user.id).await;
            let (libraries, library_access_changed) = watched.map_err(database_failed)?;
            let max_age_watched = users.maximum_age_watched(caller.user.id).await;
            let (max_age, maximum_age_changed) = max_age_watched.map_err(database_failed)?;
            let events = events
                .subscribe(
                    &caller.user,
                    libraries,
                    age_check(max_age, contents),
                    filter,
                )
                .take_until(signed_out)
                .take_until(permissions_changed)
                .take_until(library_access_changed)
                .take_until(maximum_age_changed);
            let stream: ServiceStream<SubscribeResponse> = Box::pin(events.map(Ok));
            Response::ok(stream)
        }
    }
}

/// `max_age`, judging each Matched title by the Content Rating `contents` holds for it.
fn age_check(max_age: MaximumAge, contents: Contents) -> AgeCheck {
    AgeCheck {
        max_age,
        ratings: Arc::new(move |item| {
            let contents = contents.clone();
            Box::pin(async move { contents.item_content_rating(item).await })
        }),
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
