///Shorthand for `OwnedView<GetServerInfoRequestView<'static>>`.
pub type OwnedGetServerInfoRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetServerInfoRequestView<'static>,
>;
///Shorthand for `OwnedView<GetServerInfoResponseView<'static>>`.
pub type OwnedGetServerInfoResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetServerInfoResponseView<'static>,
>;
///Shorthand for `OwnedView<GetServerSettingsRequestView<'static>>`.
pub type OwnedGetServerSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetServerSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<GetServerSettingsResponseView<'static>>`.
pub type OwnedGetServerSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetServerSettingsResponseView<'static>,
>;
///Shorthand for `OwnedView<UpdateServerSettingsRequestView<'static>>`.
pub type OwnedUpdateServerSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateServerSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<UpdateServerSettingsResponseView<'static>>`.
pub type OwnedUpdateServerSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateServerSettingsResponseView<'static>,
>;
///Shorthand for `OwnedView<GetPlaybackStatusRequestView<'static>>`.
pub type OwnedGetPlaybackStatusRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPlaybackStatusRequestView<'static>,
>;
///Shorthand for `OwnedView<GetPlaybackStatusResponseView<'static>>`.
pub type OwnedGetPlaybackStatusResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPlaybackStatusResponseView<'static>,
>;
///Shorthand for `OwnedView<GetPlaybackSettingsRequestView<'static>>`.
pub type OwnedGetPlaybackSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<GetPlaybackSettingsResponseView<'static>>`.
pub type OwnedGetPlaybackSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsResponseView<'static>,
>;
///Shorthand for `OwnedView<UpdatePlaybackSettingsRequestView<'static>>`.
pub type OwnedUpdatePlaybackSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<UpdatePlaybackSettingsResponseView<'static>>`.
pub type OwnedUpdatePlaybackSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsResponseView<'static>,
>;
///Shorthand for `OwnedView<GetImageSettingsRequestView<'static>>`.
pub type OwnedGetImageSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetImageSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<GetImageSettingsResponseView<'static>>`.
pub type OwnedGetImageSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetImageSettingsResponseView<'static>,
>;
///Shorthand for `OwnedView<UpdateImageSettingsRequestView<'static>>`.
pub type OwnedUpdateImageSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateImageSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<UpdateImageSettingsResponseView<'static>>`.
pub type OwnedUpdateImageSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateImageSettingsResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetServerInfoResponse>
for crate::messages::nama::v1::__buffa::view::GetServerInfoResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetServerInfoResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetServerInfoResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetServerSettingsResponse>
for crate::messages::nama::v1::__buffa::view::GetServerSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetServerSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetServerSettingsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdateServerSettingsResponse>
for crate::messages::nama::v1::__buffa::view::UpdateServerSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdateServerSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateServerSettingsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetPlaybackStatusResponse>
for crate::messages::nama::v1::__buffa::view::GetPlaybackStatusResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetPlaybackStatusResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPlaybackStatusResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetPlaybackSettingsResponse>
for crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetPlaybackSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdatePlaybackSettingsResponse>
for crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdatePlaybackSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetImageSettingsResponse>
for crate::messages::nama::v1::__buffa::view::GetImageSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetImageSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetImageSettingsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdateImageSettingsResponse>
for crate::messages::nama::v1::__buffa::view::UpdateImageSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdateImageSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateImageSettingsResponseView<'static>,
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
pub const SERVER_SERVICE_SERVICE_NAME: &str = "nama.v1.ServerService";
/// Static [`Spec`](::connectrpc::Spec) for the `GetServerInfo` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_GET_SERVER_INFO_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/GetServerInfo",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetServerSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_GET_SERVER_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/GetServerSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `UpdateServerSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_UPDATE_SERVER_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/UpdateServerSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `GetPlaybackStatus` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_GET_PLAYBACK_STATUS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/GetPlaybackStatus",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetPlaybackSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_GET_PLAYBACK_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/GetPlaybackSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `UpdatePlaybackSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_UPDATE_PLAYBACK_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/UpdatePlaybackSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `GetImageSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_GET_IMAGE_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/GetImageSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `UpdateImageSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_SERVICE_UPDATE_IMAGE_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.ServerService/UpdateImageSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Facts about a Server that any client can read, before signing in, and the Server's settings.
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
pub trait ServerService: Send + Sync + 'static {
    /// Returns the Server's name and the version of Nama it runs.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_server_info<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetServerInfoRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetServerInfoResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns the Server's settings. Needs `PERMISSION_MANAGE_SETTINGS`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_server_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetServerSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetServerSettingsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Replaces the Server's settings. Needs `PERMISSION_MANAGE_SETTINGS`. A `url` that isn't an http
    /// or https URL, or has a query or fragment, is refused with `REFUSAL_REASON_NOT_A_URL` naming it,
    /// and nothing changes.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn update_server_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::UpdateServerSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::UpdateServerSettingsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns the ffmpeg and ffprobe the Server found at startup, which it needs to Remux and
    /// Transcode, or why it can't use them; without both it only Direct Plays. Also returns the
    /// hardware acceleration that passed its test at startup. Needs `PERMISSION_ADMIN`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_playback_status<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetPlaybackStatusRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetPlaybackStatusResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns how the Server plays Versions it changes. Needs `PERMISSION_ADMIN`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_playback_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetPlaybackSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetPlaybackSettingsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Replaces how the Server plays Versions it changes, for plans made from then on, and for
    /// running ones too for `keep_behind_minutes`. Needs `PERMISSION_ADMIN`. A `segment_folder` that
    /// isn't an absolute path is refused with `REFUSAL_REASON_FOLDER_NOT_ABSOLUTE` naming it, a
    /// `max_transcodes` or `keep_behind_minutes` set to 0 with `REFUSAL_REASON_FIELD_REQUIRED` naming
    /// it, and nothing changes.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn update_playback_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::UpdatePlaybackSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::UpdatePlaybackSettingsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns how the Server resizes Posters and Backdrops. Needs `PERMISSION_MANAGE_SETTINGS`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_image_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetImageSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetImageSettingsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Replaces how the Server resizes Posters and Backdrops, from the next image asked for. Needs
    /// `PERMISSION_MANAGE_SETTINGS`. A `resized_cache_mib` set to 0 is refused with
    /// `REFUSAL_REASON_FIELD_REQUIRED` naming it, and nothing changes.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn update_image_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::UpdateImageSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::UpdateImageSettingsResponse,
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
pub trait ServerServiceExt: ServerService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: ServerService> ServerServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                SERVER_SERVICE_SERVICE_NAME,
                "GetServerInfo",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetServerInfoRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetServerInfoRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_server_info(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetServerInfoResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_GET_SERVER_INFO_SPEC)
            .route_view_idempotent(
                SERVER_SERVICE_SERVICE_NAME,
                "GetServerSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetServerSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetServerSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_server_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetServerSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_GET_SERVER_SETTINGS_SPEC)
            .route_view(
                SERVER_SERVICE_SERVICE_NAME,
                "UpdateServerSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::UpdateServerSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::UpdateServerSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.update_server_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::UpdateServerSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_UPDATE_SERVER_SETTINGS_SPEC)
            .route_view_idempotent(
                SERVER_SERVICE_SERVICE_NAME,
                "GetPlaybackStatus",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetPlaybackStatusRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetPlaybackStatusRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_playback_status(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetPlaybackStatusResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_GET_PLAYBACK_STATUS_SPEC)
            .route_view_idempotent(
                SERVER_SERVICE_SERVICE_NAME,
                "GetPlaybackSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetPlaybackSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_playback_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetPlaybackSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_GET_PLAYBACK_SETTINGS_SPEC)
            .route_view(
                SERVER_SERVICE_SERVICE_NAME,
                "UpdatePlaybackSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::UpdatePlaybackSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.update_playback_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::UpdatePlaybackSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_UPDATE_PLAYBACK_SETTINGS_SPEC)
            .route_view_idempotent(
                SERVER_SERVICE_SERVICE_NAME,
                "GetImageSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetImageSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetImageSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_image_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetImageSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_GET_IMAGE_SETTINGS_SPEC)
            .route_view(
                SERVER_SERVICE_SERVICE_NAME,
                "UpdateImageSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::UpdateImageSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::UpdateImageSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.update_image_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::UpdateImageSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_SERVICE_UPDATE_IMAGE_SETTINGS_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct ServerServiceRegisterMarker;
impl<S: ServerService> ::connectrpc::ServiceRegister<ServerServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as ServerServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `ServerService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = ServerServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct ServerServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: ServerService> ServerServiceServer<T> {
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
impl<T> Clone for ServerServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: ServerService> ::connectrpc::Dispatcher for ServerServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.ServerService/")?;
        match method {
            "GetServerInfo" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SERVER_SERVICE_GET_SERVER_INFO_SPEC),
                )
            }
            "GetServerSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SERVER_SERVICE_GET_SERVER_SETTINGS_SPEC),
                )
            }
            "UpdateServerSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SERVER_SERVICE_UPDATE_SERVER_SETTINGS_SPEC),
                )
            }
            "GetPlaybackStatus" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SERVER_SERVICE_GET_PLAYBACK_STATUS_SPEC),
                )
            }
            "GetPlaybackSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SERVER_SERVICE_GET_PLAYBACK_SETTINGS_SPEC),
                )
            }
            "UpdatePlaybackSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SERVER_SERVICE_UPDATE_PLAYBACK_SETTINGS_SPEC),
                )
            }
            "GetImageSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SERVER_SERVICE_GET_IMAGE_SETTINGS_SPEC),
                )
            }
            "UpdateImageSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SERVER_SERVICE_UPDATE_IMAGE_SETTINGS_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.ServerService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "GetServerInfo" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetServerInfoRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetServerInfoRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetServerInfoRequest,
                    >::from_parts(&req, &body);
                    svc.get_server_info(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::GetServerInfoResponse,
                        >(format)
                })
            }
            "GetServerSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetServerSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetServerSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetServerSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.get_server_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::GetServerSettingsResponse,
                        >(format)
                })
            }
            "UpdateServerSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::UpdateServerSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::UpdateServerSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::UpdateServerSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.update_server_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::UpdateServerSettingsResponse,
                        >(format)
                })
            }
            "GetPlaybackStatus" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetPlaybackStatusRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetPlaybackStatusRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetPlaybackStatusRequest,
                    >::from_parts(&req, &body);
                    svc.get_playback_status(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::GetPlaybackStatusResponse,
                        >(format)
                })
            }
            "GetPlaybackSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetPlaybackSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetPlaybackSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.get_playback_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::GetPlaybackSettingsResponse,
                        >(format)
                })
            }
            "UpdatePlaybackSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::UpdatePlaybackSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::UpdatePlaybackSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.update_playback_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::UpdatePlaybackSettingsResponse,
                        >(format)
                })
            }
            "GetImageSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetImageSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetImageSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetImageSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.get_image_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::GetImageSettingsResponse,
                        >(format)
                })
            }
            "UpdateImageSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::UpdateImageSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::UpdateImageSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::UpdateImageSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.update_image_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::UpdateImageSettingsResponse,
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
        let Some(method) = path.strip_prefix("nama.v1.ServerService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.ServerService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.ServerService/") else {
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
/// let client = ServerServiceClient::new(conn, config);
/// let response = client.get_server_info(request).await?;
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
/// let client = ServerServiceClient::new(http, config);
/// let response = client.get_server_info(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.get_server_info(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.get_server_info(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct ServerServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> ServerServiceClient<T>
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
    /// Call the GetServerInfo RPC. Sends a request to /nama.v1.ServerService/GetServerInfo.
    pub async fn get_server_info(
        &self,
        request: crate::messages::nama::v1::GetServerInfoRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetServerInfoResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_server_info_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetServerInfo RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_server_info_with_options(
        &self,
        request: crate::messages::nama::v1::GetServerInfoRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetServerInfoResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_GET_SERVER_INFO_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetServerSettings RPC. Sends a request to /nama.v1.ServerService/GetServerSettings.
    pub async fn get_server_settings(
        &self,
        request: crate::messages::nama::v1::GetServerSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetServerSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_server_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetServerSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_server_settings_with_options(
        &self,
        request: crate::messages::nama::v1::GetServerSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetServerSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_GET_SERVER_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UpdateServerSettings RPC. Sends a request to /nama.v1.ServerService/UpdateServerSettings.
    pub async fn update_server_settings(
        &self,
        request: crate::messages::nama::v1::UpdateServerSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdateServerSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.update_server_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UpdateServerSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn update_server_settings_with_options(
        &self,
        request: crate::messages::nama::v1::UpdateServerSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdateServerSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_UPDATE_SERVER_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetPlaybackStatus RPC. Sends a request to /nama.v1.ServerService/GetPlaybackStatus.
    pub async fn get_playback_status(
        &self,
        request: crate::messages::nama::v1::GetPlaybackStatusRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetPlaybackStatusResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_playback_status_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetPlaybackStatus RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_playback_status_with_options(
        &self,
        request: crate::messages::nama::v1::GetPlaybackStatusRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetPlaybackStatusResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_GET_PLAYBACK_STATUS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetPlaybackSettings RPC. Sends a request to /nama.v1.ServerService/GetPlaybackSettings.
    pub async fn get_playback_settings(
        &self,
        request: crate::messages::nama::v1::GetPlaybackSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_playback_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetPlaybackSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_playback_settings_with_options(
        &self,
        request: crate::messages::nama::v1::GetPlaybackSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetPlaybackSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_GET_PLAYBACK_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UpdatePlaybackSettings RPC. Sends a request to /nama.v1.ServerService/UpdatePlaybackSettings.
    pub async fn update_playback_settings(
        &self,
        request: crate::messages::nama::v1::UpdatePlaybackSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.update_playback_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UpdatePlaybackSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn update_playback_settings_with_options(
        &self,
        request: crate::messages::nama::v1::UpdatePlaybackSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdatePlaybackSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_UPDATE_PLAYBACK_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetImageSettings RPC. Sends a request to /nama.v1.ServerService/GetImageSettings.
    pub async fn get_image_settings(
        &self,
        request: crate::messages::nama::v1::GetImageSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetImageSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_image_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetImageSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_image_settings_with_options(
        &self,
        request: crate::messages::nama::v1::GetImageSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetImageSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_GET_IMAGE_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UpdateImageSettings RPC. Sends a request to /nama.v1.ServerService/UpdateImageSettings.
    pub async fn update_image_settings(
        &self,
        request: crate::messages::nama::v1::UpdateImageSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdateImageSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.update_image_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UpdateImageSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn update_image_settings_with_options(
        &self,
        request: crate::messages::nama::v1::UpdateImageSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdateImageSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_SERVICE_UPDATE_IMAGE_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
