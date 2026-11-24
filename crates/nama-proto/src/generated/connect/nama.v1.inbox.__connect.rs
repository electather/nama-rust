///Shorthand for `OwnedView<ListInboxNotificationsRequestView<'static>>`.
pub type OwnedListInboxNotificationsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListInboxNotificationsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListInboxNotificationsResponseView<'static>>`.
pub type OwnedListInboxNotificationsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListInboxNotificationsResponseView<'static>,
>;
///Shorthand for `OwnedView<CountUnreadInboxNotificationsRequestView<'static>>`.
pub type OwnedCountUnreadInboxNotificationsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CountUnreadInboxNotificationsResponseView<'static>>`.
pub type OwnedCountUnreadInboxNotificationsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<MarkInboxNotificationReadRequestView<'static>>`.
pub type OwnedMarkInboxNotificationReadRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<MarkInboxNotificationReadResponseView<'static>>`.
pub type OwnedMarkInboxNotificationReadResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<MarkAllInboxNotificationsReadRequestView<'static>>`.
pub type OwnedMarkAllInboxNotificationsReadRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<MarkAllInboxNotificationsReadResponseView<'static>>`.
pub type OwnedMarkAllInboxNotificationsReadResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadResponseView<
        'static,
    >,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListInboxNotificationsResponse>
for crate::messages::nama::v1::__buffa::view::ListInboxNotificationsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListInboxNotificationsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListInboxNotificationsResponseView<'static>,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::messages::nama::v1::CountUnreadInboxNotificationsResponse,
>
for crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::messages::nama::v1::CountUnreadInboxNotificationsResponse,
>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::messages::nama::v1::MarkInboxNotificationReadResponse,
>
for crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::messages::nama::v1::MarkInboxNotificationReadResponse,
>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::messages::nama::v1::MarkAllInboxNotificationsReadResponse,
>
for crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::messages::nama::v1::MarkAllInboxNotificationsReadResponse,
>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
/// Full service name for this service.
pub const INBOX_SERVICE_SERVICE_NAME: &str = "nama.v1.InboxService";
/// Static [`Spec`](::connectrpc::Spec) for the `ListInboxNotifications` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const INBOX_SERVICE_LIST_INBOX_NOTIFICATIONS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.InboxService/ListInboxNotifications",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `CountUnreadInboxNotifications` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const INBOX_SERVICE_COUNT_UNREAD_INBOX_NOTIFICATIONS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.InboxService/CountUnreadInboxNotifications",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `MarkInboxNotificationRead` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const INBOX_SERVICE_MARK_INBOX_NOTIFICATION_READ_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.InboxService/MarkInboxNotificationRead",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `MarkAllInboxNotificationsRead` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const INBOX_SERVICE_MARK_ALL_INBOX_NOTIFICATIONS_READ_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.InboxService/MarkAllInboxNotificationsRead",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// The signed-in caller's inbox: each Request notification the Server told them, whether or not a
/// Notifier delivered it, but none of a kind they turned off with
/// `UserService.SetNotificationPreference`. It keeps their newest 200. A caller sees and marks only
/// their own.
///
/// # Implementing handlers
///
/// Implement methods with plain `async fn`; the returned future satisfies
/// the `Send` bound automatically.
///
/// **Unary and server-streaming requests** arrive as
/// [`ServiceRequest<'_, Req>`](::connectrpc::ServiceRequest): a zero-copy
/// view of the request plus its body, valid for the duration of the call.
/// Fields are read directly (`request.name` is a `&str` into the decoded
/// buffer) and the borrow may be held across `.await` points. Anything
/// that must outlive the call — `tokio::spawn`, channels, server state,
/// or data captured by a returned response stream — takes owned data:
/// call `request.to_owned_message()` (or copy the specific fields)
/// first.
///
/// **Client-streaming and bidi requests** arrive as
/// [`InboundStream<Req>`](::connectrpc::InboundStream) — a
/// `ServiceStream` of [`StreamMessage`](::connectrpc::StreamMessage)s.
/// Each item owns its decoded buffer and is `Send + 'static`, so items
/// can be buffered or moved into spawned tasks; read fields zero-copy
/// through the generated accessor methods (`item.name()`) or `.view()`,
/// convert with `.to_owned_message()`, or yield an item back unchanged —
/// `StreamMessage<M>` implements `Encodable<M>`.
///
/// Request types resolved through `extern_path` (e.g. well-known types
/// from another crate) use the same wrappers; the crate that owns the
/// type must be generated with buffa ≥ 0.9.0 and views enabled so the
/// backing `HasMessageView` impl exists.
///
/// The `impl Encodable<Out>` return bound accepts the owned `Out`, the
/// generated `OutView<'_>` / `OwnedOutView`,
/// [`MaybeBorrowed`](::connectrpc::MaybeBorrowed), or
/// [`PreEncoded`](::connectrpc::PreEncoded) for handlers that encode a
/// non-`'static` view internally and pass the bytes across the handler
/// boundary. View bodies are not emitted for output types mapped via
/// `extern_path` (the impl would be an orphan); return owned for
/// WKT/extern outputs.
///
/// Server-streaming and bidi-streaming methods return
/// `ServiceStream<impl Encodable<Out> + Send + use<Self>>`. The
/// `use<Self>` precise-capturing clause excludes `&self`'s lifetime and
/// the request's lifetime (unary methods use `use<'a, Self>` and may
/// borrow from `&self`), so stream items must be `'static` and cannot
/// borrow from the request. To stream view-encoded data, encode each
/// item inside the stream body and yield
/// [`PreEncoded`](::connectrpc::PreEncoded) — see its `# Streaming
/// example` doc.
#[allow(clippy::type_complexity)]
pub trait InboxService: Send + Sync + 'static {
    /// Lists the caller's notifications, newest first.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_inbox_notifications<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListInboxNotificationsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListInboxNotificationsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Counts the caller's unread notifications.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn count_unread_inbox_notifications<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::CountUnreadInboxNotificationsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::CountUnreadInboxNotificationsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Marks one of the caller's notifications read. Marking a read one changes nothing. Fails with
    /// `REFUSAL_REASON_INBOX_NOTIFICATION_NOT_FOUND` for an id that isn't one of theirs.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn mark_inbox_notification_read<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::MarkInboxNotificationReadRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::MarkInboxNotificationReadResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Marks every one of the caller's notifications read.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn mark_all_inbox_notifications_read<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::MarkAllInboxNotificationsReadRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::MarkAllInboxNotificationsReadResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
}
/// Extension trait for registering a service implementation with a Router.
///
/// This trait is automatically implemented for all types that implement the service trait.
/// Prefer [`Router::add_service`](::connectrpc::Router::add_service) for
/// top-down registration; `register` remains available for compatibility
/// and cases where the service-first call shape is more convenient.
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
///
/// let service = Arc::new(MyServiceImpl);
/// let router = service.register(Router::new());
/// ```
pub trait InboxServiceExt: InboxService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: InboxService> InboxServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                INBOX_SERVICE_SERVICE_NAME,
                "ListInboxNotifications",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListInboxNotificationsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListInboxNotificationsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_inbox_notifications(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListInboxNotificationsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(INBOX_SERVICE_LIST_INBOX_NOTIFICATIONS_SPEC)
            .route_view_idempotent(
                INBOX_SERVICE_SERVICE_NAME,
                "CountUnreadInboxNotifications",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::CountUnreadInboxNotificationsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.count_unread_inbox_notifications(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::CountUnreadInboxNotificationsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(INBOX_SERVICE_COUNT_UNREAD_INBOX_NOTIFICATIONS_SPEC)
            .route_view(
                INBOX_SERVICE_SERVICE_NAME,
                "MarkInboxNotificationRead",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::MarkInboxNotificationReadRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.mark_inbox_notification_read(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::MarkInboxNotificationReadResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(INBOX_SERVICE_MARK_INBOX_NOTIFICATION_READ_SPEC)
            .route_view(
                INBOX_SERVICE_SERVICE_NAME,
                "MarkAllInboxNotificationsRead",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::MarkAllInboxNotificationsReadRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.mark_all_inbox_notifications_read(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::MarkAllInboxNotificationsReadResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(INBOX_SERVICE_MARK_ALL_INBOX_NOTIFICATIONS_READ_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct InboxServiceRegisterMarker;
impl<S: InboxService> ::connectrpc::ServiceRegister<InboxServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as InboxServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `InboxService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = InboxServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct InboxServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: InboxService> InboxServiceServer<T> {
    /// Wrap a service implementation in a monomorphic dispatcher.
    pub fn new(service: T) -> Self {
        Self {
            inner: ::std::sync::Arc::new(service),
        }
    }
    /// Wrap an already-`Arc`'d service implementation.
    pub fn from_arc(inner: ::std::sync::Arc<T>) -> Self {
        Self { inner }
    }
}
impl<T> Clone for InboxServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: InboxService> ::connectrpc::Dispatcher for InboxServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.InboxService/")?;
        match method {
            "ListInboxNotifications" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(INBOX_SERVICE_LIST_INBOX_NOTIFICATIONS_SPEC),
                )
            }
            "CountUnreadInboxNotifications" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(INBOX_SERVICE_COUNT_UNREAD_INBOX_NOTIFICATIONS_SPEC),
                )
            }
            "MarkInboxNotificationRead" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(INBOX_SERVICE_MARK_INBOX_NOTIFICATION_READ_SPEC),
                )
            }
            "MarkAllInboxNotificationsRead" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(INBOX_SERVICE_MARK_ALL_INBOX_NOTIFICATIONS_READ_SPEC),
                )
            }
            _ => None,
        }
    }
    fn call_unary(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::Payload,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::UnaryResult {
        let Some(method) = path.strip_prefix("nama.v1.InboxService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "ListInboxNotifications" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListInboxNotificationsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListInboxNotificationsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListInboxNotificationsRequest,
                    >::from_parts(&req, &body);
                    svc.list_inbox_notifications(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ListInboxNotificationsResponse,
                        >(format)
                })
            }
            "CountUnreadInboxNotifications" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::CountUnreadInboxNotificationsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::CountUnreadInboxNotificationsRequest,
                    >::from_parts(&req, &body);
                    svc.count_unread_inbox_notifications(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::CountUnreadInboxNotificationsResponse,
                        >(format)
                })
            }
            "MarkInboxNotificationRead" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::MarkInboxNotificationReadRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::MarkInboxNotificationReadRequest,
                    >::from_parts(&req, &body);
                    svc.mark_inbox_notification_read(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::MarkInboxNotificationReadResponse,
                        >(format)
                })
            }
            "MarkAllInboxNotificationsRead" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::MarkAllInboxNotificationsReadRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::MarkAllInboxNotificationsReadRequest,
                    >::from_parts(&req, &body);
                    svc.mark_all_inbox_notifications_read(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::MarkAllInboxNotificationsReadResponse,
                        >(format)
                })
            }
            _ => ::connectrpc::dispatcher::codegen::unimplemented_unary(path),
        }
    }
    fn call_server_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        request: ::buffa::bytes::Bytes,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::StreamingResult {
        let Some(method) = path.strip_prefix("nama.v1.InboxService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_streaming(path),
        }
    }
    fn call_client_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        requests: ::connectrpc::dispatcher::codegen::RequestStream,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::UnaryResult {
        let Some(method) = path.strip_prefix("nama.v1.InboxService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &requests, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_unary(path),
        }
    }
    fn call_bidi_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        requests: ::connectrpc::dispatcher::codegen::RequestStream,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::StreamingResult {
        let Some(method) = path.strip_prefix("nama.v1.InboxService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &requests, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_streaming(path),
        }
    }
}
/// Client for this service.
///
/// Generic over `T: ClientTransport`. For **gRPC** (HTTP/2), use
/// `Http2Connection` — it has honest `poll_ready` and composes with
/// `tower::balance` for multi-connection load balancing. For **Connect
/// over HTTP/1.1** (or unknown protocol), use `HttpClient`.
///
/// # Example (gRPC / HTTP/2)
///
/// ```rust,ignore
/// use connectrpc::client::{Http2Connection, ClientConfig};
/// use connectrpc::Protocol;
///
/// let uri: http::Uri = "http://localhost:8080".parse()?;
/// let conn = Http2Connection::connect_plaintext(uri.clone()).await?.shared(1024);
/// let config = ClientConfig::new(uri).with_protocol(Protocol::Grpc);
///
/// let client = InboxServiceClient::new(conn, config);
/// let response = client.list_inbox_notifications(request).await?;
/// ```
///
/// # Example (Connect / HTTP/1.1 or ALPN)
///
/// ```rust,ignore
/// use connectrpc::client::{HttpClient, ClientConfig};
///
/// let http = HttpClient::plaintext();  // cleartext http:// only
/// let config = ClientConfig::new("http://localhost:8080".parse()?);
///
/// let client = InboxServiceClient::new(http, config);
/// let response = client.list_inbox_notifications(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.list_inbox_notifications(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.list_inbox_notifications(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct InboxServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> InboxServiceClient<T>
where
    T: ::connectrpc::client::ClientTransport,
    <T::ResponseBody as ::connectrpc::http_body::Body>::Error: ::std::fmt::Display,
{
    /// Create a new client with the given transport and configuration.
    pub fn new(transport: T, config: ::connectrpc::client::ClientConfig) -> Self {
        Self { transport, config }
    }
    /// Get the client configuration.
    pub fn config(&self) -> &::connectrpc::client::ClientConfig {
        &self.config
    }
    /// Get a mutable reference to the client configuration.
    pub fn config_mut(&mut self) -> &mut ::connectrpc::client::ClientConfig {
        &mut self.config
    }
    /// Call the ListInboxNotifications RPC. Sends a request to /nama.v1.InboxService/ListInboxNotifications.
    pub async fn list_inbox_notifications(
        &self,
        request: crate::messages::nama::v1::ListInboxNotificationsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListInboxNotificationsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_inbox_notifications_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListInboxNotifications RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_inbox_notifications_with_options(
        &self,
        request: crate::messages::nama::v1::ListInboxNotificationsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListInboxNotificationsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                INBOX_SERVICE_LIST_INBOX_NOTIFICATIONS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CountUnreadInboxNotifications RPC. Sends a request to /nama.v1.InboxService/CountUnreadInboxNotifications.
    pub async fn count_unread_inbox_notifications(
        &self,
        request: crate::messages::nama::v1::CountUnreadInboxNotificationsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.count_unread_inbox_notifications_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CountUnreadInboxNotifications RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn count_unread_inbox_notifications_with_options(
        &self,
        request: crate::messages::nama::v1::CountUnreadInboxNotificationsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CountUnreadInboxNotificationsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                INBOX_SERVICE_COUNT_UNREAD_INBOX_NOTIFICATIONS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the MarkInboxNotificationRead RPC. Sends a request to /nama.v1.InboxService/MarkInboxNotificationRead.
    pub async fn mark_inbox_notification_read(
        &self,
        request: crate::messages::nama::v1::MarkInboxNotificationReadRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.mark_inbox_notification_read_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the MarkInboxNotificationRead RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn mark_inbox_notification_read_with_options(
        &self,
        request: crate::messages::nama::v1::MarkInboxNotificationReadRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::MarkInboxNotificationReadResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                INBOX_SERVICE_MARK_INBOX_NOTIFICATION_READ_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the MarkAllInboxNotificationsRead RPC. Sends a request to /nama.v1.InboxService/MarkAllInboxNotificationsRead.
    pub async fn mark_all_inbox_notifications_read(
        &self,
        request: crate::messages::nama::v1::MarkAllInboxNotificationsReadRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.mark_all_inbox_notifications_read_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the MarkAllInboxNotificationsRead RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn mark_all_inbox_notifications_read_with_options(
        &self,
        request: crate::messages::nama::v1::MarkAllInboxNotificationsReadRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::MarkAllInboxNotificationsReadResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                INBOX_SERVICE_MARK_ALL_INBOX_NOTIFICATIONS_READ_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
