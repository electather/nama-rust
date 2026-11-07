///Shorthand for `OwnedView<CreateUserRequestView<'static>>`.
pub type OwnedCreateUserRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateUserRequestView<'static>,
>;
///Shorthand for `OwnedView<CreateUserResponseView<'static>>`.
pub type OwnedCreateUserResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateUserResponseView<'static>,
>;
///Shorthand for `OwnedView<ListUsersRequestView<'static>>`.
pub type OwnedListUsersRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListUsersRequestView<'static>,
>;
///Shorthand for `OwnedView<ListUsersResponseView<'static>>`.
pub type OwnedListUsersResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListUsersResponseView<'static>,
>;
///Shorthand for `OwnedView<SetPermissionsRequestView<'static>>`.
pub type OwnedSetPermissionsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SetPermissionsRequestView<'static>,
>;
///Shorthand for `OwnedView<SetPermissionsResponseView<'static>>`.
pub type OwnedSetPermissionsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SetPermissionsResponseView<'static>,
>;
///Shorthand for `OwnedView<DeleteUserRequestView<'static>>`.
pub type OwnedDeleteUserRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteUserRequestView<'static>,
>;
///Shorthand for `OwnedView<DeleteUserResponseView<'static>>`.
pub type OwnedDeleteUserResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteUserResponseView<'static>,
>;
///Shorthand for `OwnedView<ListDevicesRequestView<'static>>`.
pub type OwnedListDevicesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListDevicesRequestView<'static>,
>;
///Shorthand for `OwnedView<ListDevicesResponseView<'static>>`.
pub type OwnedListDevicesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListDevicesResponseView<'static>,
>;
///Shorthand for `OwnedView<RevokeDeviceRequestView<'static>>`.
pub type OwnedRevokeDeviceRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RevokeDeviceRequestView<'static>,
>;
///Shorthand for `OwnedView<RevokeDeviceResponseView<'static>>`.
pub type OwnedRevokeDeviceResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RevokeDeviceResponseView<'static>,
>;
///Shorthand for `OwnedView<ChangePasswordRequestView<'static>>`.
pub type OwnedChangePasswordRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ChangePasswordRequestView<'static>,
>;
///Shorthand for `OwnedView<ChangePasswordResponseView<'static>>`.
pub type OwnedChangePasswordResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ChangePasswordResponseView<'static>,
>;
///Shorthand for `OwnedView<ResetPasswordRequestView<'static>>`.
pub type OwnedResetPasswordRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ResetPasswordRequestView<'static>,
>;
///Shorthand for `OwnedView<ResetPasswordResponseView<'static>>`.
pub type OwnedResetPasswordResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ResetPasswordResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateUserResponse>
for crate::messages::nama::v1::__buffa::view::CreateUserResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateUserResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateUserResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListUsersResponse>
for crate::messages::nama::v1::__buffa::view::ListUsersResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListUsersResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListUsersResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::SetPermissionsResponse>
for crate::messages::nama::v1::__buffa::view::SetPermissionsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::SetPermissionsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SetPermissionsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeleteUserResponse>
for crate::messages::nama::v1::__buffa::view::DeleteUserResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeleteUserResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteUserResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListDevicesResponse>
for crate::messages::nama::v1::__buffa::view::ListDevicesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListDevicesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListDevicesResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::RevokeDeviceResponse>
for crate::messages::nama::v1::__buffa::view::RevokeDeviceResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::RevokeDeviceResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RevokeDeviceResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ChangePasswordResponse>
for crate::messages::nama::v1::__buffa::view::ChangePasswordResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ChangePasswordResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ChangePasswordResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ResetPasswordResponse>
for crate::messages::nama::v1::__buffa::view::ResetPasswordResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ResetPasswordResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ResetPasswordResponseView<'static>,
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
pub const USER_SERVICE_SERVICE_NAME: &str = "nama.v1.UserService";
/// Static [`Spec`](::connectrpc::Spec) for the `CreateUser` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_CREATE_USER_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/CreateUser",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListUsers` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_LIST_USERS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/ListUsers",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `SetPermissions` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_SET_PERMISSIONS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/SetPermissions",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `DeleteUser` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_DELETE_USER_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/DeleteUser",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `ListDevices` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_LIST_DEVICES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/ListDevices",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `RevokeDevice` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_REVOKE_DEVICE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/RevokeDevice",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ChangePassword` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_CHANGE_PASSWORD_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/ChangePassword",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ResetPassword` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const USER_SERVICE_RESET_PASSWORD_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.UserService/ResetPassword",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Managing the Server's Users and their Devices. Every RPC but `ListDevices`, `RevokeDevice` and
/// `ChangePassword` needs `PERMISSION_MANAGE_USERS`, and fails with `permission_denied` without it.
/// Only a caller with `PERMISSION_ADMIN` may `SetPermissions`, `ResetPassword` or `DeleteUser` for a
/// User with it, grant it, or set their own Permissions; others fail with `permission_denied` and a
/// Refusal naming `PERMISSION_ADMIN`. `ChangePassword`, `ResetPassword`, `SetPermissions` and
/// `DeleteUser` also need Recent Authentication, as `SignInService` describes. RPCs name Users by
/// ID, not by name.
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
pub trait UserService: Send + Sync + 'static {
    /// Creates a User, who can sign in at once, with `PERMISSION_REQUEST` and
    /// `PERMISSION_VIEW_REQUESTS`. Fails with `invalid_argument` if the name or the password breaks
    /// the rules, and `already_exists` if another User has the name, ignoring case.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_user<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::CreateUserRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::CreateUserResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists every User on the Server, oldest first.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_users<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListUsersRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListUsersResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Replaces a User's Permissions, which apply from their next request. Their Devices stay signed
    /// in. Fails with `invalid_argument` for `PERMISSION_UNSPECIFIED` or a value the Server doesn't
    /// know, `not_found` if no User has the ID, and `failed_precondition` if it would leave no User
    /// with `PERMISSION_ADMIN`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn set_permissions<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::SetPermissionsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::SetPermissionsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Deletes a User with their Devices and credentials, so they can't sign in, their sessions stop
    /// working at once and their open `EventService.Subscribe` streams end. The caller may delete
    /// themselves, which signs out the calling Device too. Fails with `not_found` if no User has the
    /// ID, so a retry after a delete that went through gets `not_found`, and `failed_precondition` if
    /// it would leave no User with `PERMISSION_ADMIN`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn delete_user<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::DeleteUserRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::DeleteUserResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists a User's Devices that haven't expired, oldest first. Any signed-in User may list their
    /// own; another User's needs `PERMISSION_MANAGE_USERS`. Fails with `not_found` if the User doesn't
    /// exist, or isn't the caller and the caller lacks that Permission, so they can't learn which IDs
    /// exist.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_devices<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListDevicesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListDevicesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Revokes one of a User's Devices, so its session stops working at once and its open
    /// `EventService.Subscribe` streams end. Revoking the calling Device signs it out, as `SignOut`
    /// does. Who may revoke whose Devices is as for `ListDevices`, and fails the same way; it also
    /// fails with `not_found` if the Device isn't that User's.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn revoke_device<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::RevokeDeviceRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::RevokeDeviceResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Changes the caller's own password. Their other Devices are revoked, as by `RevokeDevice`, and
    /// the calling Device stays signed in. Fails with `unauthenticated` if the calling Device was
    /// signed out meanwhile, and `invalid_argument` if the new password breaks the rules.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn change_password<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ChangePasswordRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ChangePasswordResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Sets a User's password, without their current one, for the caller to pass on to them. Every
    /// Device of theirs is revoked, as by `RevokeDevice`. Fails with `invalid_argument` if the new
    /// password breaks the rules, `not_found` if no User has the ID, and `failed_precondition` for the
    /// caller's own User, who uses `ChangePassword` instead to keep the calling Device signed in.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn reset_password<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ResetPasswordRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ResetPasswordResponse,
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
pub trait UserServiceExt: UserService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: UserService> UserServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                USER_SERVICE_SERVICE_NAME,
                "CreateUser",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::CreateUserRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::CreateUserRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_user(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::CreateUserResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_CREATE_USER_SPEC)
            .route_view_idempotent(
                USER_SERVICE_SERVICE_NAME,
                "ListUsers",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListUsersRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListUsersRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_users(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListUsersResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_LIST_USERS_SPEC)
            .route_view(
                USER_SERVICE_SERVICE_NAME,
                "SetPermissions",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::SetPermissionsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::SetPermissionsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.set_permissions(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::SetPermissionsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_SET_PERMISSIONS_SPEC)
            .route_view(
                USER_SERVICE_SERVICE_NAME,
                "DeleteUser",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::DeleteUserRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::DeleteUserRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.delete_user(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::DeleteUserResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_DELETE_USER_SPEC)
            .route_view_idempotent(
                USER_SERVICE_SERVICE_NAME,
                "ListDevices",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListDevicesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListDevicesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_devices(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListDevicesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_LIST_DEVICES_SPEC)
            .route_view(
                USER_SERVICE_SERVICE_NAME,
                "RevokeDevice",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::RevokeDeviceRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::RevokeDeviceRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.revoke_device(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::RevokeDeviceResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_REVOKE_DEVICE_SPEC)
            .route_view(
                USER_SERVICE_SERVICE_NAME,
                "ChangePassword",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ChangePasswordRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ChangePasswordRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.change_password(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ChangePasswordResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_CHANGE_PASSWORD_SPEC)
            .route_view(
                USER_SERVICE_SERVICE_NAME,
                "ResetPassword",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ResetPasswordRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ResetPasswordRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.reset_password(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ResetPasswordResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(USER_SERVICE_RESET_PASSWORD_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct UserServiceRegisterMarker;
impl<S: UserService> ::connectrpc::ServiceRegister<UserServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as UserServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `UserService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = UserServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct UserServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: UserService> UserServiceServer<T> {
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
impl<T> Clone for UserServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: UserService> ::connectrpc::Dispatcher for UserServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.UserService/")?;
        match method {
            "CreateUser" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(USER_SERVICE_CREATE_USER_SPEC),
                )
            }
            "ListUsers" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(USER_SERVICE_LIST_USERS_SPEC),
                )
            }
            "SetPermissions" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(USER_SERVICE_SET_PERMISSIONS_SPEC),
                )
            }
            "DeleteUser" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(USER_SERVICE_DELETE_USER_SPEC),
                )
            }
            "ListDevices" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(USER_SERVICE_LIST_DEVICES_SPEC),
                )
            }
            "RevokeDevice" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(USER_SERVICE_REVOKE_DEVICE_SPEC),
                )
            }
            "ChangePassword" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(USER_SERVICE_CHANGE_PASSWORD_SPEC),
                )
            }
            "ResetPassword" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(USER_SERVICE_RESET_PASSWORD_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.UserService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "CreateUser" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::CreateUserRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::CreateUserRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::CreateUserRequest,
                    >::from_parts(&req, &body);
                    svc.create_user(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::CreateUserResponse>(format)
                })
            }
            "ListUsers" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListUsersRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListUsersRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListUsersRequest,
                    >::from_parts(&req, &body);
                    svc.list_users(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ListUsersResponse>(format)
                })
            }
            "SetPermissions" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::SetPermissionsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::SetPermissionsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::SetPermissionsRequest,
                    >::from_parts(&req, &body);
                    svc.set_permissions(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::SetPermissionsResponse,
                        >(format)
                })
            }
            "DeleteUser" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::DeleteUserRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::DeleteUserRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::DeleteUserRequest,
                    >::from_parts(&req, &body);
                    svc.delete_user(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::DeleteUserResponse>(format)
                })
            }
            "ListDevices" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListDevicesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListDevicesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListDevicesRequest,
                    >::from_parts(&req, &body);
                    svc.list_devices(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ListDevicesResponse>(format)
                })
            }
            "RevokeDevice" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::RevokeDeviceRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::RevokeDeviceRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::RevokeDeviceRequest,
                    >::from_parts(&req, &body);
                    svc.revoke_device(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::RevokeDeviceResponse,
                        >(format)
                })
            }
            "ChangePassword" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ChangePasswordRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ChangePasswordRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ChangePasswordRequest,
                    >::from_parts(&req, &body);
                    svc.change_password(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ChangePasswordResponse,
                        >(format)
                })
            }
            "ResetPassword" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ResetPasswordRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ResetPasswordRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ResetPasswordRequest,
                    >::from_parts(&req, &body);
                    svc.reset_password(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ResetPasswordResponse,
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
        let Some(method) = path.strip_prefix("nama.v1.UserService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.UserService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.UserService/") else {
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
/// let client = UserServiceClient::new(conn, config);
/// let response = client.create_user(request).await?;
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
/// let client = UserServiceClient::new(http, config);
/// let response = client.create_user(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.create_user(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.create_user(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct UserServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> UserServiceClient<T>
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
    /// Call the CreateUser RPC. Sends a request to /nama.v1.UserService/CreateUser.
    pub async fn create_user(
        &self,
        request: crate::messages::nama::v1::CreateUserRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateUserResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_user_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateUser RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_user_with_options(
        &self,
        request: crate::messages::nama::v1::CreateUserRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateUserResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_CREATE_USER_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListUsers RPC. Sends a request to /nama.v1.UserService/ListUsers.
    pub async fn list_users(
        &self,
        request: crate::messages::nama::v1::ListUsersRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListUsersResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_users_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListUsers RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_users_with_options(
        &self,
        request: crate::messages::nama::v1::ListUsersRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListUsersResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_LIST_USERS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SetPermissions RPC. Sends a request to /nama.v1.UserService/SetPermissions.
    pub async fn set_permissions(
        &self,
        request: crate::messages::nama::v1::SetPermissionsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SetPermissionsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.set_permissions_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SetPermissions RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn set_permissions_with_options(
        &self,
        request: crate::messages::nama::v1::SetPermissionsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SetPermissionsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_SET_PERMISSIONS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the DeleteUser RPC. Sends a request to /nama.v1.UserService/DeleteUser.
    pub async fn delete_user(
        &self,
        request: crate::messages::nama::v1::DeleteUserRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeleteUserResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.delete_user_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the DeleteUser RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn delete_user_with_options(
        &self,
        request: crate::messages::nama::v1::DeleteUserRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeleteUserResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_DELETE_USER_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListDevices RPC. Sends a request to /nama.v1.UserService/ListDevices.
    pub async fn list_devices(
        &self,
        request: crate::messages::nama::v1::ListDevicesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListDevicesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_devices_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListDevices RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_devices_with_options(
        &self,
        request: crate::messages::nama::v1::ListDevicesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListDevicesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_LIST_DEVICES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the RevokeDevice RPC. Sends a request to /nama.v1.UserService/RevokeDevice.
    pub async fn revoke_device(
        &self,
        request: crate::messages::nama::v1::RevokeDeviceRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::RevokeDeviceResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.revoke_device_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the RevokeDevice RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn revoke_device_with_options(
        &self,
        request: crate::messages::nama::v1::RevokeDeviceRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::RevokeDeviceResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_REVOKE_DEVICE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ChangePassword RPC. Sends a request to /nama.v1.UserService/ChangePassword.
    pub async fn change_password(
        &self,
        request: crate::messages::nama::v1::ChangePasswordRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ChangePasswordResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.change_password_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ChangePassword RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn change_password_with_options(
        &self,
        request: crate::messages::nama::v1::ChangePasswordRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ChangePasswordResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_CHANGE_PASSWORD_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ResetPassword RPC. Sends a request to /nama.v1.UserService/ResetPassword.
    pub async fn reset_password(
        &self,
        request: crate::messages::nama::v1::ResetPasswordRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ResetPasswordResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.reset_password_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ResetPassword RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn reset_password_with_options(
        &self,
        request: crate::messages::nama::v1::ResetPasswordRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ResetPasswordResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                USER_SERVICE_RESET_PASSWORD_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
