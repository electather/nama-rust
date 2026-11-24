//! Each signed-in User's inbox of Request notifications: `nama.v1.InboxService`.

use std::sync::Arc;

use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use nama_proto::nama::v1::{
    CountUnreadInboxNotificationsRequest, CountUnreadInboxNotificationsResponse, InboxService,
    ListInboxNotificationsRequest, ListInboxNotificationsResponse,
    MarkAllInboxNotificationsReadRequest, MarkAllInboxNotificationsReadResponse,
    MarkInboxNotificationReadRequest, MarkInboxNotificationReadResponse, RefusalReason,
};

use crate::{
    db::Database,
    page::Page,
    users::{count_unread_in_inbox, list_inbox, mark_all_inbox_read, mark_inbox_read},
};

use super::{caller, database_failed, refuse};

/// `nama.v1.InboxService`: each signed-in User's own inbox, and no one else's.
pub(super) struct InboxServiceHandler {
    pub(super) db: Arc<Database>,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl InboxService for InboxServiceHandler {
    async fn list_inbox_notifications(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ListInboxNotificationsRequest>,
    ) -> ServiceResult<ListInboxNotificationsResponse> {
        let user = caller(&ctx)?.user.id;
        let request = request.to_owned_message();
        let page = Page::read(&request.page, b"ListInboxNotifications").map_err(|invalid| {
            ConnectError::from(
                refuse(
                    RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
                    invalid.to_string(),
                )
                .field("page.token"),
            )
        })?;
        let (notifications, paging) =
            (list_inbox(&self.db, user, &page).await).map_err(database_failed)?;
        Response::ok(ListInboxNotificationsResponse {
            notifications,
            page: paging.into(),
            ..Default::default()
        })
    }

    async fn count_unread_inbox_notifications(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, CountUnreadInboxNotificationsRequest>,
    ) -> ServiceResult<CountUnreadInboxNotificationsResponse> {
        let user = caller(&ctx)?.user.id;
        let unread = (count_unread_in_inbox(&self.db, user).await).map_err(database_failed)?;
        Response::ok(CountUnreadInboxNotificationsResponse {
            unread,
            ..Default::default()
        })
    }

    async fn mark_inbox_notification_read(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, MarkInboxNotificationReadRequest>,
    ) -> ServiceResult<MarkInboxNotificationReadResponse> {
        let user = caller(&ctx)?.user.id;
        let id = request.to_owned_message().id;
        let marked = (mark_inbox_read(&self.db, user, id).await).map_err(database_failed)?;
        if !marked {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_INBOX_NOTIFICATION_NOT_FOUND,
                format!("your inbox has no notification {id}"),
            )
            .field("id")
            .into());
        }
        Response::ok(MarkInboxNotificationReadResponse::default())
    }

    async fn mark_all_inbox_notifications_read(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, MarkAllInboxNotificationsReadRequest>,
    ) -> ServiceResult<MarkAllInboxNotificationsReadResponse> {
        let user = caller(&ctx)?.user.id;
        (mark_all_inbox_read(&self.db, user).await).map_err(database_failed)?;
        Response::ok(MarkAllInboxNotificationsReadResponse::default())
    }
}
