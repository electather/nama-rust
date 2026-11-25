//! Each signed-in User's Web Push subscriptions: `nama.v1.WebPushService`.

use std::sync::Arc;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    GetWebPushKeyRequest, GetWebPushKeyResponse, RefusalReason, SubscribeWebPushRequest,
    SubscribeWebPushResponse, UnsubscribeWebPushRequest, UnsubscribeWebPushResponse,
    WebPushService,
};

use crate::{
    db::Database,
    server_settings::web_push_key,
    web_push::{Invalid, Subscription, subscribe, unsubscribe},
};

use super::{caller, database_failed, refuse};

/// `nama.v1.WebPushService`: each signed-in User's own subscriptions, and no one else's.
pub(super) struct WebPushServiceHandler {
    pub(super) db: Arc<Database>,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl WebPushService for WebPushServiceHandler {
    async fn get_web_push_key(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetWebPushKeyRequest>,
    ) -> ServiceResult<GetWebPushKeyResponse> {
        let key = web_push_key(&self.db).await.map_err(database_failed)?;
        Response::ok(GetWebPushKeyResponse {
            public_key: key.public_key().to_vec(),
            ..Default::default()
        })
    }

    async fn subscribe_web_push(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SubscribeWebPushRequest>,
    ) -> ServiceResult<SubscribeWebPushResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let subscription = Subscription::new(request.endpoint, request.p256dh, request.auth)
            .map_err(|invalid| {
                let field = match invalid {
                    Invalid::Endpoint => "endpoint",
                    Invalid::P256dh => "p256dh",
                    Invalid::Auth => "auth",
                };
                let reason = RefusalReason::REFUSAL_REASON_WEB_PUSH_SUBSCRIPTION_INVALID;
                ConnectError::from(refuse(reason, invalid.to_string()).field(field))
            })?;
        (subscribe(&self.db, user, subscription).await).map_err(database_failed)?;
        Response::ok(SubscribeWebPushResponse::default())
    }

    async fn unsubscribe_web_push(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, UnsubscribeWebPushRequest>,
    ) -> ServiceResult<UnsubscribeWebPushResponse> {
        let user = caller(&ctx)?.user.id;
        let endpoint = request.to_owned_message().endpoint;
        (unsubscribe(&self.db, user, endpoint).await).map_err(database_failed)?;
        Response::ok(UnsubscribeWebPushResponse::default())
    }
}
