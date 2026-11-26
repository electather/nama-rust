///Shorthand for `OwnedView<AddToWatchlistRequestView<'static>>`.
pub type OwnedAddToWatchlistRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::AddToWatchlistRequestView<'static>,
>;
///Shorthand for `OwnedView<AddToWatchlistResponseView<'static>>`.
pub type OwnedAddToWatchlistResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::AddToWatchlistResponseView<'static>,
>;
///Shorthand for `OwnedView<RemoveFromWatchlistRequestView<'static>>`.
pub type OwnedRemoveFromWatchlistRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistRequestView<'static>,
>;
///Shorthand for `OwnedView<RemoveFromWatchlistResponseView<'static>>`.
pub type OwnedRemoveFromWatchlistResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistResponseView<'static>,
>;
///Shorthand for `OwnedView<ListWatchlistRequestView<'static>>`.
pub type OwnedListWatchlistRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListWatchlistRequestView<'static>,
>;
///Shorthand for `OwnedView<ListWatchlistResponseView<'static>>`.
pub type OwnedListWatchlistResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListWatchlistResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::AddToWatchlistResponse>
for crate::messages::nama::v1::__buffa::view::AddToWatchlistResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::AddToWatchlistResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::AddToWatchlistResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::RemoveFromWatchlistResponse>
for crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::RemoveFromWatchlistResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListWatchlistResponse>
for crate::messages::nama::v1::__buffa::view::ListWatchlistResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListWatchlistResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListWatchlistResponseView<'static>,
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
pub const WATCHLIST_SERVICE_SERVICE_NAME: &str = "nama.v1.WatchlistService";
/// Static [`Spec`](::connectrpc::Spec) for the `AddToWatchlist` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const WATCHLIST_SERVICE_ADD_TO_WATCHLIST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.WatchlistService/AddToWatchlist",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `RemoveFromWatchlist` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const WATCHLIST_SERVICE_REMOVE_FROM_WATCHLIST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.WatchlistService/RemoveFromWatchlist",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `ListWatchlist` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const WATCHLIST_SERVICE_LIST_WATCHLIST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.WatchlistService/ListWatchlist",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// The signed-in caller's Watchlist (ADR 0056): the Movies and Shows they mean to watch.
///
/// A Watchlist keeps each title by its kind and external id, whether or not the Server has it, so
/// a title missing from every Library can be added too. `BrowseService` says whether a Movie or
/// Show is on the caller's Watchlist, under any of its ids, in `Movie.on_watchlist` and
/// `Show.on_watchlist`.
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
pub trait WatchlistService: Send + Sync + 'static {
    /// Adds a title to the caller's Watchlist. Adding one already on it changes nothing. Fails with
    /// `invalid_argument` if the kind is unspecified or the id missing or malformed.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn add_to_watchlist<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::AddToWatchlistRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::AddToWatchlistResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Removes a title from the caller's Watchlist, by the id it was added under. Removing one that
    /// isn't on it changes nothing. Fails as `AddToWatchlist` does.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn remove_from_watchlist<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::RemoveFromWatchlistRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::RemoveFromWatchlistResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists the caller's Watchlist, newest added first, each title as a Home row shows it: on the
    /// Server in the oldest Library they see that has it, or else missing as the title source has it
    /// now, if they may Request its kind. One only Libraries they don't see have, that their Maximum
    /// Age keeps out, or that the title source doesn't have or can't be asked about now is left out.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_watchlist<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListWatchlistRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListWatchlistResponse,
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
pub trait WatchlistServiceExt: WatchlistService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: WatchlistService> WatchlistServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                WATCHLIST_SERVICE_SERVICE_NAME,
                "AddToWatchlist",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::AddToWatchlistRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::AddToWatchlistRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.add_to_watchlist(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::AddToWatchlistResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(WATCHLIST_SERVICE_ADD_TO_WATCHLIST_SPEC)
            .route_view(
                WATCHLIST_SERVICE_SERVICE_NAME,
                "RemoveFromWatchlist",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::RemoveFromWatchlistRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.remove_from_watchlist(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::RemoveFromWatchlistResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(WATCHLIST_SERVICE_REMOVE_FROM_WATCHLIST_SPEC)
            .route_view_idempotent(
                WATCHLIST_SERVICE_SERVICE_NAME,
                "ListWatchlist",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListWatchlistRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListWatchlistRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_watchlist(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListWatchlistResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(WATCHLIST_SERVICE_LIST_WATCHLIST_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct WatchlistServiceRegisterMarker;
impl<S: WatchlistService> ::connectrpc::ServiceRegister<WatchlistServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as WatchlistServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `WatchlistService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = WatchlistServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct WatchlistServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: WatchlistService> WatchlistServiceServer<T> {
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
impl<T> Clone for WatchlistServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: WatchlistService> ::connectrpc::Dispatcher for WatchlistServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.WatchlistService/")?;
        match method {
            "AddToWatchlist" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(WATCHLIST_SERVICE_ADD_TO_WATCHLIST_SPEC),
                )
            }
            "RemoveFromWatchlist" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(WATCHLIST_SERVICE_REMOVE_FROM_WATCHLIST_SPEC),
                )
            }
            "ListWatchlist" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(WATCHLIST_SERVICE_LIST_WATCHLIST_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.WatchlistService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "AddToWatchlist" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::AddToWatchlistRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::AddToWatchlistRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::AddToWatchlistRequest,
                    >::from_parts(&req, &body);
                    svc.add_to_watchlist(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::AddToWatchlistResponse,
                        >(format)
                })
            }
            "RemoveFromWatchlist" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::RemoveFromWatchlistRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::RemoveFromWatchlistRequest,
                    >::from_parts(&req, &body);
                    svc.remove_from_watchlist(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::RemoveFromWatchlistResponse,
                        >(format)
                })
            }
            "ListWatchlist" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListWatchlistRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListWatchlistRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListWatchlistRequest,
                    >::from_parts(&req, &body);
                    svc.list_watchlist(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ListWatchlistResponse,
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
        let Some(method) = path.strip_prefix("nama.v1.WatchlistService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.WatchlistService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.WatchlistService/") else {
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
/// let client = WatchlistServiceClient::new(conn, config);
/// let response = client.add_to_watchlist(request).await?;
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
/// let client = WatchlistServiceClient::new(http, config);
/// let response = client.add_to_watchlist(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.add_to_watchlist(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.add_to_watchlist(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct WatchlistServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> WatchlistServiceClient<T>
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
    /// Call the AddToWatchlist RPC. Sends a request to /nama.v1.WatchlistService/AddToWatchlist.
    pub async fn add_to_watchlist(
        &self,
        request: crate::messages::nama::v1::AddToWatchlistRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::AddToWatchlistResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.add_to_watchlist_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the AddToWatchlist RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn add_to_watchlist_with_options(
        &self,
        request: crate::messages::nama::v1::AddToWatchlistRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::AddToWatchlistResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                WATCHLIST_SERVICE_ADD_TO_WATCHLIST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the RemoveFromWatchlist RPC. Sends a request to /nama.v1.WatchlistService/RemoveFromWatchlist.
    pub async fn remove_from_watchlist(
        &self,
        request: crate::messages::nama::v1::RemoveFromWatchlistRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.remove_from_watchlist_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the RemoveFromWatchlist RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn remove_from_watchlist_with_options(
        &self,
        request: crate::messages::nama::v1::RemoveFromWatchlistRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::RemoveFromWatchlistResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                WATCHLIST_SERVICE_REMOVE_FROM_WATCHLIST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListWatchlist RPC. Sends a request to /nama.v1.WatchlistService/ListWatchlist.
    pub async fn list_watchlist(
        &self,
        request: crate::messages::nama::v1::ListWatchlistRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListWatchlistResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_watchlist_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListWatchlist RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_watchlist_with_options(
        &self,
        request: crate::messages::nama::v1::ListWatchlistRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListWatchlistResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                WATCHLIST_SERVICE_LIST_WATCHLIST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
