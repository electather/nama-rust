///Shorthand for `OwnedView<SetupRequestView<'static>>`.
pub type OwnedSetupRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SetupRequestView<'static>,
>;
///Shorthand for `OwnedView<SetupResponseView<'static>>`.
pub type OwnedSetupResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SetupResponseView<'static>,
>;
///Shorthand for `OwnedView<SignInRequestView<'static>>`.
pub type OwnedSignInRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SignInRequestView<'static>,
>;
///Shorthand for `OwnedView<SignInResponseView<'static>>`.
pub type OwnedSignInResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SignInResponseView<'static>,
>;
///Shorthand for `OwnedView<SignOutRequestView<'static>>`.
pub type OwnedSignOutRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SignOutRequestView<'static>,
>;
///Shorthand for `OwnedView<SignOutResponseView<'static>>`.
pub type OwnedSignOutResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SignOutResponseView<'static>,
>;
///Shorthand for `OwnedView<WhoAmIRequestView<'static>>`.
pub type OwnedWhoAmIRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::WhoAmIRequestView<'static>,
>;
///Shorthand for `OwnedView<WhoAmIResponseView<'static>>`.
pub type OwnedWhoAmIResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::WhoAmIResponseView<'static>,
>;
///Shorthand for `OwnedView<StartSignInCodeRequestView<'static>>`.
pub type OwnedStartSignInCodeRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::StartSignInCodeRequestView<'static>,
>;
///Shorthand for `OwnedView<StartSignInCodeResponseView<'static>>`.
pub type OwnedStartSignInCodeResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::StartSignInCodeResponseView<'static>,
>;
///Shorthand for `OwnedView<LookUpSignInCodeRequestView<'static>>`.
pub type OwnedLookUpSignInCodeRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::LookUpSignInCodeRequestView<'static>,
>;
///Shorthand for `OwnedView<LookUpSignInCodeResponseView<'static>>`.
pub type OwnedLookUpSignInCodeResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::LookUpSignInCodeResponseView<'static>,
>;
///Shorthand for `OwnedView<ApproveSignInCodeRequestView<'static>>`.
pub type OwnedApproveSignInCodeRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ApproveSignInCodeRequestView<'static>,
>;
///Shorthand for `OwnedView<ApproveSignInCodeResponseView<'static>>`.
pub type OwnedApproveSignInCodeResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ApproveSignInCodeResponseView<'static>,
>;
///Shorthand for `OwnedView<PollSignInCodeRequestView<'static>>`.
pub type OwnedPollSignInCodeRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PollSignInCodeRequestView<'static>,
>;
///Shorthand for `OwnedView<PollSignInCodeResponseView<'static>>`.
pub type OwnedPollSignInCodeResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PollSignInCodeResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::SetupResponse>
for crate::messages::nama::v1::__buffa::view::SetupResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::SetupResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SetupResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::SignInResponse>
for crate::messages::nama::v1::__buffa::view::SignInResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::SignInResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SignInResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::SignOutResponse>
for crate::messages::nama::v1::__buffa::view::SignOutResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::SignOutResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SignOutResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::WhoAmIResponse>
for crate::messages::nama::v1::__buffa::view::WhoAmIResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::WhoAmIResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::WhoAmIResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::StartSignInCodeResponse>
for crate::messages::nama::v1::__buffa::view::StartSignInCodeResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::StartSignInCodeResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::StartSignInCodeResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::LookUpSignInCodeResponse>
for crate::messages::nama::v1::__buffa::view::LookUpSignInCodeResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::LookUpSignInCodeResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::LookUpSignInCodeResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ApproveSignInCodeResponse>
for crate::messages::nama::v1::__buffa::view::ApproveSignInCodeResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ApproveSignInCodeResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ApproveSignInCodeResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::PollSignInCodeResponse>
for crate::messages::nama::v1::__buffa::view::PollSignInCodeResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::PollSignInCodeResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PollSignInCodeResponseView<'static>,
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
pub const SIGN_IN_SERVICE_SERVICE_NAME: &str = "nama.v1.SignInService";
/// Static [`Spec`](::connectrpc::Spec) for the `Setup` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_SETUP_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/Setup",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `SignIn` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_SIGN_IN_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/SignIn",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `SignOut` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_SIGN_OUT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/SignOut",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `WhoAmI` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_WHO_AM_I_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/WhoAmI",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `StartSignInCode` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/StartSignInCode",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `LookUpSignInCode` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_LOOK_UP_SIGN_IN_CODE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/LookUpSignInCode",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ApproveSignInCode` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/ApproveSignInCode",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `PollSignInCode` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.SignInService/PollSignInCode",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Signing Devices in to a Server as Users, and finding out who is signed in (ADR 0014).
///
/// A signed-in Device holds a session: a cookie for browsers, or a bearer token (`Authorization:
/// Bearer \<token\>`) for other clients. Either one identifies the same User and Device on every
/// request.
///
/// A Device that goes unused for 90 days expires, and its session stops working.
///
/// Guessing is slowed down: after 10 failed `SignIn`s for one name, or 50 failed `Setup`s and
/// `SignIn`s together from one IP address, within 15 minutes, further attempts fail with
/// `resource_exhausted`, without being checked, and a `Retry-After` header gives the seconds to
/// wait. A successful `SignIn` clears its name's failures. In the same way, after 10 failed
/// `LookUpSignInCode`s and `ApproveSignInCode`s together from one Device within 15 minutes, its
/// further lookups and approvals fail with `resource_exhausted`.
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
pub trait SignInService: Send + Sync + 'static {
    /// Creates the first Admin on a fresh Server and signs the calling Device in as them. Needs the
    /// Setup Code the Server printed to its log. Fails with `permission_denied` if the code is wrong,
    /// `failed_precondition` once the Server has any User, and `invalid_argument` if a name or the
    /// password breaks the rules.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn setup<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::SetupRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::SetupResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Signs the calling Device in as an existing User, found by name ignoring case. Fails with
    /// `unauthenticated` if the name or the password is wrong, without saying which, and with
    /// `invalid_argument` if the Device name breaks the rules.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn sign_in<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::SignInRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::SignInResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Signs the calling Device out by deleting it: its session stops working at once, its open
    /// `EventService.Subscribe` streams end, and a cookie session's cookie is cleared. Fails with
    /// `unauthenticated` without a valid session.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn sign_out<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::SignOutRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::SignOutResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns the calling User and Device, or fails with `unauthenticated` without a valid session.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn who_am_i<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::WhoAmIRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::WhoAmIResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Starts signing the calling Device in without a password typed on it. It shows the returned
    /// Sign-in Code, a User signed in on another Device approves it with `ApproveSignInCode`, and this
    /// Device then picks up its session with `PollSignInCode`. The code and the poll handle expire
    /// after 10 minutes, and a Server restart cancels them. Fails with `invalid_argument` if the
    /// Device name breaks the rules, and `resource_exhausted` while too many sign-ins are waiting.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn start_sign_in_code<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::StartSignInCodeRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::StartSignInCodeResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns the name of the Device a Sign-in Code would sign in, so the User can check it before
    /// approving. Approves nothing. Fails with `not_found` if no unexpired code matches,
    /// `failed_precondition` if it was already approved, and `unauthenticated` without a valid
    /// session.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn look_up_sign_in_code<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::LookUpSignInCodeRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::LookUpSignInCodeResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Approves a Sign-in Code, so the Device that shows it signs in as the calling User. Fails like
    /// `LookUpSignInCode`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn approve_sign_in_code<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ApproveSignInCodeRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ApproveSignInCodeResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Says whether the sign-in a poll handle stands for is still waiting for approval, expired, or
    /// signed in. Signed in hands out the new Device's session, once, the way `SignIn` does: after
    /// that, and for a handle the Server never gave out or has forgotten, it fails with `not_found`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn poll_sign_in_code<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::PollSignInCodeRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::PollSignInCodeResponse,
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
pub trait SignInServiceExt: SignInService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: SignInService> SignInServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "Setup",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::SetupRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::SetupRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.setup(ctx, sreq)
                                .await?
                                .encode::<crate::messages::nama::v1::SetupResponse>(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_SETUP_SPEC)
            .route_view(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "SignIn",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::SignInRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::SignInRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.sign_in(ctx, sreq)
                                .await?
                                .encode::<crate::messages::nama::v1::SignInResponse>(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_SIGN_IN_SPEC)
            .route_view(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "SignOut",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::SignOutRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::SignOutRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.sign_out(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::SignOutResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_SIGN_OUT_SPEC)
            .route_view_idempotent(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "WhoAmI",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::WhoAmIRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::WhoAmIRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.who_am_i(ctx, sreq)
                                .await?
                                .encode::<crate::messages::nama::v1::WhoAmIResponse>(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_WHO_AM_I_SPEC)
            .route_view(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "StartSignInCode",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::StartSignInCodeRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::StartSignInCodeRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.start_sign_in_code(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::StartSignInCodeResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC)
            .route_view(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "LookUpSignInCode",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::LookUpSignInCodeRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::LookUpSignInCodeRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.look_up_sign_in_code(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::LookUpSignInCodeResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_LOOK_UP_SIGN_IN_CODE_SPEC)
            .route_view(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "ApproveSignInCode",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ApproveSignInCodeRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ApproveSignInCodeRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.approve_sign_in_code(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ApproveSignInCodeResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC)
            .route_view(
                SIGN_IN_SERVICE_SERVICE_NAME,
                "PollSignInCode",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::PollSignInCodeRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::PollSignInCodeRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.poll_sign_in_code(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::PollSignInCodeResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct SignInServiceRegisterMarker;
impl<S: SignInService> ::connectrpc::ServiceRegister<SignInServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as SignInServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `SignInService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = SignInServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct SignInServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: SignInService> SignInServiceServer<T> {
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
impl<T> Clone for SignInServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: SignInService> ::connectrpc::Dispatcher for SignInServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.SignInService/")?;
        match method {
            "Setup" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIGN_IN_SERVICE_SETUP_SPEC),
                )
            }
            "SignIn" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIGN_IN_SERVICE_SIGN_IN_SPEC),
                )
            }
            "SignOut" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIGN_IN_SERVICE_SIGN_OUT_SPEC),
                )
            }
            "WhoAmI" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SIGN_IN_SERVICE_WHO_AM_I_SPEC),
                )
            }
            "StartSignInCode" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC),
                )
            }
            "LookUpSignInCode" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIGN_IN_SERVICE_LOOK_UP_SIGN_IN_CODE_SPEC),
                )
            }
            "ApproveSignInCode" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC),
                )
            }
            "PollSignInCode" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.SignInService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "Setup" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::SetupRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::SetupRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::SetupRequest,
                    >::from_parts(&req, &body);
                    svc.setup(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::SetupResponse>(format)
                })
            }
            "SignIn" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::SignInRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::SignInRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::SignInRequest,
                    >::from_parts(&req, &body);
                    svc.sign_in(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::SignInResponse>(format)
                })
            }
            "SignOut" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::SignOutRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::SignOutRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::SignOutRequest,
                    >::from_parts(&req, &body);
                    svc.sign_out(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::SignOutResponse>(format)
                })
            }
            "WhoAmI" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::WhoAmIRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::WhoAmIRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::WhoAmIRequest,
                    >::from_parts(&req, &body);
                    svc.who_am_i(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::WhoAmIResponse>(format)
                })
            }
            "StartSignInCode" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::StartSignInCodeRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::StartSignInCodeRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::StartSignInCodeRequest,
                    >::from_parts(&req, &body);
                    svc.start_sign_in_code(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::StartSignInCodeResponse,
                        >(format)
                })
            }
            "LookUpSignInCode" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::LookUpSignInCodeRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::LookUpSignInCodeRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::LookUpSignInCodeRequest,
                    >::from_parts(&req, &body);
                    svc.look_up_sign_in_code(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::LookUpSignInCodeResponse,
                        >(format)
                })
            }
            "ApproveSignInCode" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ApproveSignInCodeRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ApproveSignInCodeRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ApproveSignInCodeRequest,
                    >::from_parts(&req, &body);
                    svc.approve_sign_in_code(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ApproveSignInCodeResponse,
                        >(format)
                })
            }
            "PollSignInCode" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::PollSignInCodeRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::PollSignInCodeRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::PollSignInCodeRequest,
                    >::from_parts(&req, &body);
                    svc.poll_sign_in_code(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::PollSignInCodeResponse,
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
        let Some(method) = path.strip_prefix("nama.v1.SignInService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.SignInService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.SignInService/") else {
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
/// let client = SignInServiceClient::new(conn, config);
/// let response = client.setup(request).await?;
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
/// let client = SignInServiceClient::new(http, config);
/// let response = client.setup(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.setup(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.setup(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct SignInServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> SignInServiceClient<T>
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
    /// Call the Setup RPC. Sends a request to /nama.v1.SignInService/Setup.
    pub async fn setup(
        &self,
        request: crate::messages::nama::v1::SetupRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SetupResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.setup_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Setup RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn setup_with_options(
        &self,
        request: crate::messages::nama::v1::SetupRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SetupResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_SETUP_SPEC.with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SignIn RPC. Sends a request to /nama.v1.SignInService/SignIn.
    pub async fn sign_in(
        &self,
        request: crate::messages::nama::v1::SignInRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SignInResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.sign_in_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the SignIn RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn sign_in_with_options(
        &self,
        request: crate::messages::nama::v1::SignInRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SignInResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_SIGN_IN_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SignOut RPC. Sends a request to /nama.v1.SignInService/SignOut.
    pub async fn sign_out(
        &self,
        request: crate::messages::nama::v1::SignOutRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SignOutResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.sign_out_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the SignOut RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn sign_out_with_options(
        &self,
        request: crate::messages::nama::v1::SignOutRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SignOutResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_SIGN_OUT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the WhoAmI RPC. Sends a request to /nama.v1.SignInService/WhoAmI.
    pub async fn who_am_i(
        &self,
        request: crate::messages::nama::v1::WhoAmIRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::WhoAmIResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.who_am_i_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the WhoAmI RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn who_am_i_with_options(
        &self,
        request: crate::messages::nama::v1::WhoAmIRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::WhoAmIResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_WHO_AM_I_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the StartSignInCode RPC. Sends a request to /nama.v1.SignInService/StartSignInCode.
    pub async fn start_sign_in_code(
        &self,
        request: crate::messages::nama::v1::StartSignInCodeRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::StartSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.start_sign_in_code_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the StartSignInCode RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn start_sign_in_code_with_options(
        &self,
        request: crate::messages::nama::v1::StartSignInCodeRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::StartSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_START_SIGN_IN_CODE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the LookUpSignInCode RPC. Sends a request to /nama.v1.SignInService/LookUpSignInCode.
    pub async fn look_up_sign_in_code(
        &self,
        request: crate::messages::nama::v1::LookUpSignInCodeRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::LookUpSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.look_up_sign_in_code_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the LookUpSignInCode RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn look_up_sign_in_code_with_options(
        &self,
        request: crate::messages::nama::v1::LookUpSignInCodeRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::LookUpSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_LOOK_UP_SIGN_IN_CODE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ApproveSignInCode RPC. Sends a request to /nama.v1.SignInService/ApproveSignInCode.
    pub async fn approve_sign_in_code(
        &self,
        request: crate::messages::nama::v1::ApproveSignInCodeRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ApproveSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.approve_sign_in_code_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ApproveSignInCode RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn approve_sign_in_code_with_options(
        &self,
        request: crate::messages::nama::v1::ApproveSignInCodeRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ApproveSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_APPROVE_SIGN_IN_CODE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the PollSignInCode RPC. Sends a request to /nama.v1.SignInService/PollSignInCode.
    pub async fn poll_sign_in_code(
        &self,
        request: crate::messages::nama::v1::PollSignInCodeRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::PollSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.poll_sign_in_code_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the PollSignInCode RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn poll_sign_in_code_with_options(
        &self,
        request: crate::messages::nama::v1::PollSignInCodeRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::PollSignInCodeResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIGN_IN_SERVICE_POLL_SIGN_IN_CODE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
