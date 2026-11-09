///Shorthand for `OwnedView<ListPluginsRequestView<'static>>`.
pub type OwnedListPluginsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListPluginsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListPluginsResponseView<'static>>`.
pub type OwnedListPluginsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListPluginsResponseView<'static>,
>;
///Shorthand for `OwnedView<GetPluginRequestView<'static>>`.
pub type OwnedGetPluginRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPluginRequestView<'static>,
>;
///Shorthand for `OwnedView<GetPluginResponseView<'static>>`.
pub type OwnedGetPluginResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPluginResponseView<'static>,
>;
///Shorthand for `OwnedView<UpdatePluginSettingsRequestView<'static>>`.
pub type OwnedUpdatePluginSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<UpdatePluginSettingsResponseView<'static>>`.
pub type OwnedUpdatePluginSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsResponseView<'static>,
>;
///Shorthand for `OwnedView<EnablePluginRequestView<'static>>`.
pub type OwnedEnablePluginRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::EnablePluginRequestView<'static>,
>;
///Shorthand for `OwnedView<EnablePluginResponseView<'static>>`.
pub type OwnedEnablePluginResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::EnablePluginResponseView<'static>,
>;
///Shorthand for `OwnedView<TestPluginSettingsRequestView<'static>>`.
pub type OwnedTestPluginSettingsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::TestPluginSettingsRequestView<'static>,
>;
///Shorthand for `OwnedView<TestPluginSettingsResponseView<'static>>`.
pub type OwnedTestPluginSettingsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::TestPluginSettingsResponseView<'static>,
>;
///Shorthand for `OwnedView<DisablePluginRequestView<'static>>`.
pub type OwnedDisablePluginRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DisablePluginRequestView<'static>,
>;
///Shorthand for `OwnedView<DisablePluginResponseView<'static>>`.
pub type OwnedDisablePluginResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DisablePluginResponseView<'static>,
>;
///Shorthand for `OwnedView<ListQualitiesRequestView<'static>>`.
pub type OwnedListQualitiesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListQualitiesRequestView<'static>,
>;
///Shorthand for `OwnedView<ListQualitiesResponseView<'static>>`.
pub type OwnedListQualitiesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListQualitiesResponseView<'static>,
>;
///Shorthand for `OwnedView<ReplaceQualitiesRequestView<'static>>`.
pub type OwnedReplaceQualitiesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ReplaceQualitiesRequestView<'static>,
>;
///Shorthand for `OwnedView<ReplaceQualitiesResponseView<'static>>`.
pub type OwnedReplaceQualitiesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ReplaceQualitiesResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListPluginsResponse>
for crate::messages::nama::v1::__buffa::view::ListPluginsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListPluginsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListPluginsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetPluginResponse>
for crate::messages::nama::v1::__buffa::view::GetPluginResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetPluginResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetPluginResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdatePluginSettingsResponse>
for crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdatePluginSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::EnablePluginResponse>
for crate::messages::nama::v1::__buffa::view::EnablePluginResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::EnablePluginResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::EnablePluginResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::TestPluginSettingsResponse>
for crate::messages::nama::v1::__buffa::view::TestPluginSettingsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::TestPluginSettingsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::TestPluginSettingsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::DisablePluginResponse>
for crate::messages::nama::v1::__buffa::view::DisablePluginResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::DisablePluginResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DisablePluginResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListQualitiesResponse>
for crate::messages::nama::v1::__buffa::view::ListQualitiesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListQualitiesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListQualitiesResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ReplaceQualitiesResponse>
for crate::messages::nama::v1::__buffa::view::ReplaceQualitiesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ReplaceQualitiesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ReplaceQualitiesResponseView<'static>,
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
pub const PLUGIN_SERVICE_SERVICE_NAME: &str = "nama.v1.PluginService";
/// Static [`Spec`](::connectrpc::Spec) for the `ListPlugins` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_LIST_PLUGINS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/ListPlugins",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetPlugin` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_GET_PLUGIN_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/GetPlugin",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `UpdatePluginSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_UPDATE_PLUGIN_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/UpdatePluginSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `EnablePlugin` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_ENABLE_PLUGIN_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/EnablePlugin",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `TestPluginSettings` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_TEST_PLUGIN_SETTINGS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/TestPluginSettings",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `DisablePlugin` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_DISABLE_PLUGIN_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/DisablePlugin",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `ListQualities` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_LIST_QUALITIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/ListQualities",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ReplaceQualities` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const PLUGIN_SERVICE_REPLACE_QUALITIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.PluginService/ReplaceQualities",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// The Server's Plugins and their settings. Needs `PERMISSION_MANAGE_SETTINGS`, and fails with
/// `permission_denied` without it. A secret setting's value never leaves the Server. Every call
/// but `GetPlugin` naming a Plugin the Server refused (see `Plugin.refused_reason`) fails with
/// `REFUSAL_REASON_PLUGIN_REFUSED`.
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
pub trait PluginService: Send + Sync + 'static {
    /// Lists every Plugin on the Server, by id, without its settings.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_plugins<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListPluginsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListPluginsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns one Plugin with its settings. A refused Plugin comes with its `refused_reason` and no
    /// settings. Fails with `not_found` for an unknown Plugin.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_plugin<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetPluginRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetPluginResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Replaces a Plugin's settings and, if it is meant to be enabled, enables it again with them,
    /// without a restart. Settings that don't fit its schema are refused with a Refusal naming
    /// `settings.<key>`, and nothing changes. If a choice setting has a value, the Plugin's settings
    /// check runs on the new settings first, and a check that fails, or doesn't offer that value, is
    /// refused with `REFUSAL_REASON_SETTINGS_CHECK_FAILED` or `REFUSAL_REASON_CHOICE_NOT_OFFERED`
    /// naming the setting, and nothing changes. Fails with `not_found` for an unknown Plugin.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn update_plugin_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::UpdatePluginSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::UpdatePluginSettingsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Enables a Plugin with its stored settings, without a restart, and saves that it is enabled.
    /// Enabling one that is enabled already only saves that. If its stored settings don't fit its
    /// schema, fails with `REFUSAL_REASON_FIELD_REQUIRED` or `REFUSAL_REASON_SETTING_WRONG_KIND`,
    /// naming the setting in `field` as `settings.<key>` and in the `settingKey` param; the Plugin
    /// stays disabled and nothing is saved.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn enable_plugin<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::EnablePluginRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::EnablePluginResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Runs a Plugin's settings check on `settings`, which may not be saved yet, and returns the
    /// choices it offers for each `SETTING_KIND_CHOICE` setting. It runs only when called, never to
    /// enable a Plugin. Settings that don't fit the Plugin's schema are refused as in
    /// `UpdatePluginSettings`, except that a choice setting may be left out. A check that fails, runs
    /// too long or crashes is refused with `REFUSAL_REASON_SETTINGS_CHECK_FAILED`, its message saying
    /// why in plain words, as is a Plugin without a check (see `Plugin.checks_settings`). Fails with
    /// `not_found` for an unknown Plugin.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn test_plugin_settings<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::TestPluginSettingsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::TestPluginSettingsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Disables a Plugin, removing everything it registered, without a restart, and saves that it is
    /// disabled. Fails with `REFUSAL_REASON_LAST_AUTHENTICATOR` if no other enabled Plugin provides an
    /// Authenticator; it stays enabled and nothing is saved.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn disable_plugin<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::DisablePluginRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::DisablePluginResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns a Fulfiller's Qualities (ADR 0025), in the order an admin listed them. Empty until an
    /// admin lists some, which leaves the Plugin's own `quality_profile` setting in use. Fails with
    /// `not_found` for an unknown Plugin.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_qualities<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListQualitiesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListQualitiesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Replaces a Fulfiller's Qualities. A list with any Qualities needs exactly one default, else
    /// it is refused with `REFUSAL_REASON_DEFAULT_QUALITY_REQUIRED`; a blank name or profile with
    /// `REFUSAL_REASON_FIELD_REQUIRED`; and a profile or name (ignoring case) given twice with
    /// `REFUSAL_REASON_QUALITY_REPEATED`. The Plugin's settings check then runs on its stored
    /// settings: a check that fails is refused with `REFUSAL_REASON_SETTINGS_CHECK_FAILED`, and a
    /// profile it doesn't offer for `quality_profile` with `REFUSAL_REASON_CHOICE_NOT_OFFERED`. Each
    /// refusal names the field, such as `qualities[1].profile_id`, and nothing changes. Clearing the
    /// list runs no check. Fails with `not_found` for an unknown Plugin.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn replace_qualities<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ReplaceQualitiesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ReplaceQualitiesResponse,
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
pub trait PluginServiceExt: PluginService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: PluginService> PluginServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                PLUGIN_SERVICE_SERVICE_NAME,
                "ListPlugins",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListPluginsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListPluginsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_plugins(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListPluginsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_LIST_PLUGINS_SPEC)
            .route_view_idempotent(
                PLUGIN_SERVICE_SERVICE_NAME,
                "GetPlugin",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetPluginRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetPluginRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_plugin(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetPluginResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_GET_PLUGIN_SPEC)
            .route_view(
                PLUGIN_SERVICE_SERVICE_NAME,
                "UpdatePluginSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::UpdatePluginSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.update_plugin_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::UpdatePluginSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_UPDATE_PLUGIN_SETTINGS_SPEC)
            .route_view(
                PLUGIN_SERVICE_SERVICE_NAME,
                "EnablePlugin",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::EnablePluginRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::EnablePluginRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.enable_plugin(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::EnablePluginResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_ENABLE_PLUGIN_SPEC)
            .route_view(
                PLUGIN_SERVICE_SERVICE_NAME,
                "TestPluginSettings",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::TestPluginSettingsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::TestPluginSettingsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.test_plugin_settings(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::TestPluginSettingsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_TEST_PLUGIN_SETTINGS_SPEC)
            .route_view(
                PLUGIN_SERVICE_SERVICE_NAME,
                "DisablePlugin",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::DisablePluginRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::DisablePluginRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.disable_plugin(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::DisablePluginResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_DISABLE_PLUGIN_SPEC)
            .route_view_idempotent(
                PLUGIN_SERVICE_SERVICE_NAME,
                "ListQualities",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListQualitiesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListQualitiesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_qualities(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListQualitiesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_LIST_QUALITIES_SPEC)
            .route_view(
                PLUGIN_SERVICE_SERVICE_NAME,
                "ReplaceQualities",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ReplaceQualitiesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ReplaceQualitiesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.replace_qualities(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ReplaceQualitiesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(PLUGIN_SERVICE_REPLACE_QUALITIES_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct PluginServiceRegisterMarker;
impl<S: PluginService> ::connectrpc::ServiceRegister<PluginServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as PluginServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `PluginService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = PluginServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct PluginServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: PluginService> PluginServiceServer<T> {
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
impl<T> Clone for PluginServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: PluginService> ::connectrpc::Dispatcher for PluginServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.PluginService/")?;
        match method {
            "ListPlugins" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(PLUGIN_SERVICE_LIST_PLUGINS_SPEC),
                )
            }
            "GetPlugin" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(PLUGIN_SERVICE_GET_PLUGIN_SPEC),
                )
            }
            "UpdatePluginSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(PLUGIN_SERVICE_UPDATE_PLUGIN_SETTINGS_SPEC),
                )
            }
            "EnablePlugin" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(PLUGIN_SERVICE_ENABLE_PLUGIN_SPEC),
                )
            }
            "TestPluginSettings" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(PLUGIN_SERVICE_TEST_PLUGIN_SETTINGS_SPEC),
                )
            }
            "DisablePlugin" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(PLUGIN_SERVICE_DISABLE_PLUGIN_SPEC),
                )
            }
            "ListQualities" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(PLUGIN_SERVICE_LIST_QUALITIES_SPEC),
                )
            }
            "ReplaceQualities" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(PLUGIN_SERVICE_REPLACE_QUALITIES_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.PluginService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "ListPlugins" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListPluginsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListPluginsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListPluginsRequest,
                    >::from_parts(&req, &body);
                    svc.list_plugins(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ListPluginsResponse>(format)
                })
            }
            "GetPlugin" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetPluginRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetPluginRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetPluginRequest,
                    >::from_parts(&req, &body);
                    svc.get_plugin(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::GetPluginResponse>(format)
                })
            }
            "UpdatePluginSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::UpdatePluginSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::UpdatePluginSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.update_plugin_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::UpdatePluginSettingsResponse,
                        >(format)
                })
            }
            "EnablePlugin" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::EnablePluginRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::EnablePluginRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::EnablePluginRequest,
                    >::from_parts(&req, &body);
                    svc.enable_plugin(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::EnablePluginResponse,
                        >(format)
                })
            }
            "TestPluginSettings" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::TestPluginSettingsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::TestPluginSettingsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::TestPluginSettingsRequest,
                    >::from_parts(&req, &body);
                    svc.test_plugin_settings(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::TestPluginSettingsResponse,
                        >(format)
                })
            }
            "DisablePlugin" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::DisablePluginRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::DisablePluginRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::DisablePluginRequest,
                    >::from_parts(&req, &body);
                    svc.disable_plugin(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::DisablePluginResponse,
                        >(format)
                })
            }
            "ListQualities" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListQualitiesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListQualitiesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListQualitiesRequest,
                    >::from_parts(&req, &body);
                    svc.list_qualities(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ListQualitiesResponse,
                        >(format)
                })
            }
            "ReplaceQualities" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ReplaceQualitiesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ReplaceQualitiesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ReplaceQualitiesRequest,
                    >::from_parts(&req, &body);
                    svc.replace_qualities(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ReplaceQualitiesResponse,
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
        let Some(method) = path.strip_prefix("nama.v1.PluginService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.PluginService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.PluginService/") else {
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
/// let client = PluginServiceClient::new(conn, config);
/// let response = client.list_plugins(request).await?;
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
/// let client = PluginServiceClient::new(http, config);
/// let response = client.list_plugins(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.list_plugins(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.list_plugins(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct PluginServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> PluginServiceClient<T>
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
    /// Call the ListPlugins RPC. Sends a request to /nama.v1.PluginService/ListPlugins.
    pub async fn list_plugins(
        &self,
        request: crate::messages::nama::v1::ListPluginsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListPluginsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_plugins_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListPlugins RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_plugins_with_options(
        &self,
        request: crate::messages::nama::v1::ListPluginsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListPluginsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_LIST_PLUGINS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetPlugin RPC. Sends a request to /nama.v1.PluginService/GetPlugin.
    pub async fn get_plugin(
        &self,
        request: crate::messages::nama::v1::GetPluginRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetPluginResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_plugin_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetPlugin RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_plugin_with_options(
        &self,
        request: crate::messages::nama::v1::GetPluginRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetPluginResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_GET_PLUGIN_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UpdatePluginSettings RPC. Sends a request to /nama.v1.PluginService/UpdatePluginSettings.
    pub async fn update_plugin_settings(
        &self,
        request: crate::messages::nama::v1::UpdatePluginSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.update_plugin_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UpdatePluginSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn update_plugin_settings_with_options(
        &self,
        request: crate::messages::nama::v1::UpdatePluginSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdatePluginSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_UPDATE_PLUGIN_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the EnablePlugin RPC. Sends a request to /nama.v1.PluginService/EnablePlugin.
    pub async fn enable_plugin(
        &self,
        request: crate::messages::nama::v1::EnablePluginRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::EnablePluginResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.enable_plugin_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the EnablePlugin RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn enable_plugin_with_options(
        &self,
        request: crate::messages::nama::v1::EnablePluginRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::EnablePluginResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_ENABLE_PLUGIN_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the TestPluginSettings RPC. Sends a request to /nama.v1.PluginService/TestPluginSettings.
    pub async fn test_plugin_settings(
        &self,
        request: crate::messages::nama::v1::TestPluginSettingsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::TestPluginSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.test_plugin_settings_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the TestPluginSettings RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn test_plugin_settings_with_options(
        &self,
        request: crate::messages::nama::v1::TestPluginSettingsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::TestPluginSettingsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_TEST_PLUGIN_SETTINGS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the DisablePlugin RPC. Sends a request to /nama.v1.PluginService/DisablePlugin.
    pub async fn disable_plugin(
        &self,
        request: crate::messages::nama::v1::DisablePluginRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DisablePluginResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.disable_plugin_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the DisablePlugin RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn disable_plugin_with_options(
        &self,
        request: crate::messages::nama::v1::DisablePluginRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DisablePluginResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_DISABLE_PLUGIN_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListQualities RPC. Sends a request to /nama.v1.PluginService/ListQualities.
    pub async fn list_qualities(
        &self,
        request: crate::messages::nama::v1::ListQualitiesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListQualitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_qualities_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListQualities RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_qualities_with_options(
        &self,
        request: crate::messages::nama::v1::ListQualitiesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListQualitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_LIST_QUALITIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ReplaceQualities RPC. Sends a request to /nama.v1.PluginService/ReplaceQualities.
    pub async fn replace_qualities(
        &self,
        request: crate::messages::nama::v1::ReplaceQualitiesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ReplaceQualitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.replace_qualities_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ReplaceQualities RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn replace_qualities_with_options(
        &self,
        request: crate::messages::nama::v1::ReplaceQualitiesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ReplaceQualitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                PLUGIN_SERVICE_REPLACE_QUALITIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
