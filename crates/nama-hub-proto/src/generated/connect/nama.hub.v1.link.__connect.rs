///Shorthand for `OwnedView<StartServerLinkRequestView<'static>>`.
pub type OwnedStartServerLinkRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::StartServerLinkRequestView<'static>,
>;
///Shorthand for `OwnedView<StartServerLinkResponseView<'static>>`.
pub type OwnedStartServerLinkResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::StartServerLinkResponseView<'static>,
>;
///Shorthand for `OwnedView<PollServerLinkRequestView<'static>>`.
pub type OwnedPollServerLinkRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::PollServerLinkRequestView<'static>,
>;
///Shorthand for `OwnedView<PollServerLinkResponseView<'static>>`.
pub type OwnedPollServerLinkResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::PollServerLinkResponseView<'static>,
>;
///Shorthand for `OwnedView<ApproveServerLinkRequestView<'static>>`.
pub type OwnedApproveServerLinkRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkRequestView<'static>,
>;
///Shorthand for `OwnedView<ApproveServerLinkResponseView<'static>>`.
pub type OwnedApproveServerLinkResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkResponseView<'static>,
>;
///Shorthand for `OwnedView<ListLinkedServersRequestView<'static>>`.
pub type OwnedListLinkedServersRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersRequestView<'static>,
>;
///Shorthand for `OwnedView<ListLinkedServersResponseView<'static>>`.
pub type OwnedListLinkedServersResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersResponseView<'static>,
>;
///Shorthand for `OwnedView<ApproveUserLinkRequestView<'static>>`.
pub type OwnedApproveUserLinkRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkRequestView<'static>,
>;
///Shorthand for `OwnedView<ApproveUserLinkResponseView<'static>>`.
pub type OwnedApproveUserLinkResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkResponseView<'static>,
>;
///Shorthand for `OwnedView<ListLinkedUsersRequestView<'static>>`.
pub type OwnedListLinkedUsersRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersRequestView<'static>,
>;
///Shorthand for `OwnedView<ListLinkedUsersResponseView<'static>>`.
pub type OwnedListLinkedUsersResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersResponseView<'static>,
>;
///Shorthand for `OwnedView<UnlinkLinkedUserRequestView<'static>>`.
pub type OwnedUnlinkLinkedUserRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserRequestView<'static>,
>;
///Shorthand for `OwnedView<UnlinkLinkedUserResponseView<'static>>`.
pub type OwnedUnlinkLinkedUserResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserResponseView<'static>,
>;
///Shorthand for `OwnedView<UnlinkServerRequestView<'static>>`.
pub type OwnedUnlinkServerRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkServerRequestView<'static>,
>;
///Shorthand for `OwnedView<UnlinkServerResponseView<'static>>`.
pub type OwnedUnlinkServerResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkServerResponseView<'static>,
>;
///Shorthand for `OwnedView<StartUserLinkRequestView<'static>>`.
pub type OwnedStartUserLinkRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::StartUserLinkRequestView<'static>,
>;
///Shorthand for `OwnedView<StartUserLinkResponseView<'static>>`.
pub type OwnedStartUserLinkResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::StartUserLinkResponseView<'static>,
>;
///Shorthand for `OwnedView<GetUserLinkRequestView<'static>>`.
pub type OwnedGetUserLinkRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::GetUserLinkRequestView<'static>,
>;
///Shorthand for `OwnedView<GetUserLinkResponseView<'static>>`.
pub type OwnedGetUserLinkResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::GetUserLinkResponseView<'static>,
>;
///Shorthand for `OwnedView<UnlinkUserRequestView<'static>>`.
pub type OwnedUnlinkUserRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkUserRequestView<'static>,
>;
///Shorthand for `OwnedView<UnlinkUserResponseView<'static>>`.
pub type OwnedUnlinkUserResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkUserResponseView<'static>,
>;
///Shorthand for `OwnedView<PushWatchStatesRequestView<'static>>`.
pub type OwnedPushWatchStatesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesRequestView<'static>,
>;
///Shorthand for `OwnedView<PushWatchStatesResponseView<'static>>`.
pub type OwnedPushWatchStatesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::StartServerLinkResponse>
for crate::messages::nama::hub::v1::__buffa::view::StartServerLinkResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::StartServerLinkResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::StartServerLinkResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::PollServerLinkResponse>
for crate::messages::nama::hub::v1::__buffa::view::PollServerLinkResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::PollServerLinkResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::PollServerLinkResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ApproveServerLinkResponse>
for crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ApproveServerLinkResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ListLinkedServersResponse>
for crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ListLinkedServersResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ApproveUserLinkResponse>
for crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ApproveUserLinkResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ListLinkedUsersResponse>
for crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::ListLinkedUsersResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::UnlinkLinkedUserResponse>
for crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::UnlinkLinkedUserResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::UnlinkServerResponse>
for crate::messages::nama::hub::v1::__buffa::view::UnlinkServerResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::UnlinkServerResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkServerResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::StartUserLinkResponse>
for crate::messages::nama::hub::v1::__buffa::view::StartUserLinkResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::StartUserLinkResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::StartUserLinkResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::GetUserLinkResponse>
for crate::messages::nama::hub::v1::__buffa::view::GetUserLinkResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::GetUserLinkResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::GetUserLinkResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::UnlinkUserResponse>
for crate::messages::nama::hub::v1::__buffa::view::UnlinkUserResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::UnlinkUserResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::UnlinkUserResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::PushWatchStatesResponse>
for crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::hub::v1::PushWatchStatesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesResponseView<'static>,
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
pub const SERVER_LINK_SERVICE_SERVICE_NAME: &str = "nama.hub.v1.ServerLinkService";
/// Static [`Spec`](::connectrpc::Spec) for the `StartServerLink` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_LINK_SERVICE_START_SERVER_LINK_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.ServerLinkService/StartServerLink",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `PollServerLink` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SERVER_LINK_SERVICE_POLL_SERVER_LINK_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.ServerLinkService/PollServerLink",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Registering a Server with the Hub, as an OAuth device flow (RFC 8628, ADR 0011): the Server
/// starts it and polls, and the holder of an Account enters the code it shows on the Hub's pages.
/// The Hub never calls a Server. Anyone may call it, since a Server not yet Linked has no identity.
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
pub trait ServerLinkService: Send + Sync + 'static {
    /// Starts registering the calling Server: the code a person enters on the Hub, and the device code
    /// the Server polls with. Both stop working after `expires_in_seconds`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn start_server_link<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::StartServerLinkRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::StartServerLinkResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Whether someone entered the code yet. Once they have, the first poll gets the Server's
    /// credential and every later one fails. Fails with `REFUSAL_REASON_LINK_EXPIRED` for a device
    /// code that expired, was used, or never existed.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn poll_server_link<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::PollServerLinkRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::PollServerLinkResponse,
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
pub trait ServerLinkServiceExt: ServerLinkService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: ServerLinkService> ServerLinkServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                SERVER_LINK_SERVICE_SERVICE_NAME,
                "StartServerLink",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::StartServerLinkRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::StartServerLinkRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.start_server_link(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::StartServerLinkResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_LINK_SERVICE_START_SERVER_LINK_SPEC)
            .route_view(
                SERVER_LINK_SERVICE_SERVICE_NAME,
                "PollServerLink",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::PollServerLinkRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::PollServerLinkRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.poll_server_link(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::PollServerLinkResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SERVER_LINK_SERVICE_POLL_SERVER_LINK_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct ServerLinkServiceRegisterMarker;
impl<S: ServerLinkService> ::connectrpc::ServiceRegister<ServerLinkServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as ServerLinkServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `ServerLinkService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = ServerLinkServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct ServerLinkServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: ServerLinkService> ServerLinkServiceServer<T> {
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
impl<T> Clone for ServerLinkServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: ServerLinkService> ::connectrpc::Dispatcher for ServerLinkServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.hub.v1.ServerLinkService/")?;
        match method {
            "StartServerLink" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SERVER_LINK_SERVICE_START_SERVER_LINK_SPEC),
                )
            }
            "PollServerLink" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SERVER_LINK_SERVICE_POLL_SERVER_LINK_SPEC),
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
        let Some(method) = path.strip_prefix("nama.hub.v1.ServerLinkService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "StartServerLink" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::StartServerLinkRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::StartServerLinkRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::StartServerLinkRequest,
                    >::from_parts(&req, &body);
                    svc.start_server_link(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::StartServerLinkResponse,
                        >(format)
                })
            }
            "PollServerLink" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::PollServerLinkRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::PollServerLinkRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::PollServerLinkRequest,
                    >::from_parts(&req, &body);
                    svc.poll_server_link(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::PollServerLinkResponse,
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
        let Some(method) = path.strip_prefix("nama.hub.v1.ServerLinkService/") else {
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
        let Some(method) = path.strip_prefix("nama.hub.v1.ServerLinkService/") else {
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
        let Some(method) = path.strip_prefix("nama.hub.v1.ServerLinkService/") else {
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
/// let client = ServerLinkServiceClient::new(conn, config);
/// let response = client.start_server_link(request).await?;
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
/// let client = ServerLinkServiceClient::new(http, config);
/// let response = client.start_server_link(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.start_server_link(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.start_server_link(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct ServerLinkServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> ServerLinkServiceClient<T>
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
    /// Call the StartServerLink RPC. Sends a request to /nama.hub.v1.ServerLinkService/StartServerLink.
    pub async fn start_server_link(
        &self,
        request: crate::messages::nama::hub::v1::StartServerLinkRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::StartServerLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.start_server_link_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the StartServerLink RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn start_server_link_with_options(
        &self,
        request: crate::messages::nama::hub::v1::StartServerLinkRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::StartServerLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_LINK_SERVICE_START_SERVER_LINK_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the PollServerLink RPC. Sends a request to /nama.hub.v1.ServerLinkService/PollServerLink.
    pub async fn poll_server_link(
        &self,
        request: crate::messages::nama::hub::v1::PollServerLinkRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::PollServerLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.poll_server_link_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the PollServerLink RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn poll_server_link_with_options(
        &self,
        request: crate::messages::nama::hub::v1::PollServerLinkRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::PollServerLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SERVER_LINK_SERVICE_POLL_SERVER_LINK_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
/// Full service name for this service.
pub const LINK_SERVICE_SERVICE_NAME: &str = "nama.hub.v1.LinkService";
/// Static [`Spec`](::connectrpc::Spec) for the `ApproveServerLink` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINK_SERVICE_APPROVE_SERVER_LINK_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkService/ApproveServerLink",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListLinkedServers` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINK_SERVICE_LIST_LINKED_SERVERS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkService/ListLinkedServers",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ApproveUserLink` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINK_SERVICE_APPROVE_USER_LINK_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkService/ApproveUserLink",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListLinkedUsers` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINK_SERVICE_LIST_LINKED_USERS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkService/ListLinkedUsers",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `UnlinkLinkedUser` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINK_SERVICE_UNLINK_LINKED_USER_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkService/UnlinkLinkedUser",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Linking a Server to the signed-in Account, and listing the Servers it owns, for the Hub's web
/// pages. Every call fails with `REFUSAL_REASON_SIGN_IN_REQUIRED` without a session.
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
pub trait LinkService: Send + Sync + 'static {
    /// Makes the Server showing `code` a Linked Server of the signed-in Account. Fails with
    /// `REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED` unless the session signed in within the last
    /// 10 minutes, with `REFUSAL_REASON_EMAIL_NOT_PROVEN` until a Sign-in Link proved the Account's
    /// email (ADR 0100), and with `REFUSAL_REASON_LINK_CODE_NOT_FOUND` for a code no Server is waiting
    /// on.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn approve_server_link<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::ApproveServerLinkRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::ApproveServerLinkResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns the signed-in Account's Linked Servers, oldest first.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_linked_servers<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::ListLinkedServersRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::ListLinkedServersResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Links the User showing `code` on their Server to the signed-in Account. An Account may have
    /// Users on many Servers. Fails like `ApproveServerLink`, with `REFUSAL_REASON_LINK_CODE_NOT_FOUND`
    /// for a code no User is waiting on.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn approve_user_link<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::ApproveUserLinkRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::ApproveUserLinkResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns the signed-in Account's Linked Users, oldest first.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_linked_users<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::ListLinkedUsersRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::ListLinkedUsersResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Unlinks one of the signed-in Account's Users. Unlinking one it doesn't have does nothing.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn unlink_linked_user<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::UnlinkLinkedUserRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::UnlinkLinkedUserResponse,
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
pub trait LinkServiceExt: LinkService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: LinkService> LinkServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                LINK_SERVICE_SERVICE_NAME,
                "ApproveServerLink",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::ApproveServerLinkRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.approve_server_link(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::ApproveServerLinkResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINK_SERVICE_APPROVE_SERVER_LINK_SPEC)
            .route_view_idempotent(
                LINK_SERVICE_SERVICE_NAME,
                "ListLinkedServers",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::ListLinkedServersRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_linked_servers(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::ListLinkedServersResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINK_SERVICE_LIST_LINKED_SERVERS_SPEC)
            .route_view(
                LINK_SERVICE_SERVICE_NAME,
                "ApproveUserLink",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::ApproveUserLinkRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.approve_user_link(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::ApproveUserLinkResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINK_SERVICE_APPROVE_USER_LINK_SPEC)
            .route_view_idempotent(
                LINK_SERVICE_SERVICE_NAME,
                "ListLinkedUsers",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::ListLinkedUsersRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_linked_users(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::ListLinkedUsersResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINK_SERVICE_LIST_LINKED_USERS_SPEC)
            .route_view(
                LINK_SERVICE_SERVICE_NAME,
                "UnlinkLinkedUser",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::UnlinkLinkedUserRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.unlink_linked_user(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::UnlinkLinkedUserResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINK_SERVICE_UNLINK_LINKED_USER_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct LinkServiceRegisterMarker;
impl<S: LinkService> ::connectrpc::ServiceRegister<LinkServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as LinkServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `LinkService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = LinkServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct LinkServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: LinkService> LinkServiceServer<T> {
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
impl<T> Clone for LinkServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: LinkService> ::connectrpc::Dispatcher for LinkServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.hub.v1.LinkService/")?;
        match method {
            "ApproveServerLink" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LINK_SERVICE_APPROVE_SERVER_LINK_SPEC),
                )
            }
            "ListLinkedServers" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(LINK_SERVICE_LIST_LINKED_SERVERS_SPEC),
                )
            }
            "ApproveUserLink" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LINK_SERVICE_APPROVE_USER_LINK_SPEC),
                )
            }
            "ListLinkedUsers" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(LINK_SERVICE_LIST_LINKED_USERS_SPEC),
                )
            }
            "UnlinkLinkedUser" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LINK_SERVICE_UNLINK_LINKED_USER_SPEC),
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "ApproveServerLink" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::ApproveServerLinkRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::ApproveServerLinkRequest,
                    >::from_parts(&req, &body);
                    svc.approve_server_link(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::ApproveServerLinkResponse,
                        >(format)
                })
            }
            "ListLinkedServers" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::ListLinkedServersRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::ListLinkedServersRequest,
                    >::from_parts(&req, &body);
                    svc.list_linked_servers(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::ListLinkedServersResponse,
                        >(format)
                })
            }
            "ApproveUserLink" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::ApproveUserLinkRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::ApproveUserLinkRequest,
                    >::from_parts(&req, &body);
                    svc.approve_user_link(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::ApproveUserLinkResponse,
                        >(format)
                })
            }
            "ListLinkedUsers" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::ListLinkedUsersRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::ListLinkedUsersRequest,
                    >::from_parts(&req, &body);
                    svc.list_linked_users(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::ListLinkedUsersResponse,
                        >(format)
                })
            }
            "UnlinkLinkedUser" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::UnlinkLinkedUserRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::UnlinkLinkedUserRequest,
                    >::from_parts(&req, &body);
                    svc.unlink_linked_user(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::UnlinkLinkedUserResponse,
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkService/") else {
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkService/") else {
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkService/") else {
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
/// let client = LinkServiceClient::new(conn, config);
/// let response = client.approve_server_link(request).await?;
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
/// let client = LinkServiceClient::new(http, config);
/// let response = client.approve_server_link(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.approve_server_link(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.approve_server_link(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct LinkServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> LinkServiceClient<T>
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
    /// Call the ApproveServerLink RPC. Sends a request to /nama.hub.v1.LinkService/ApproveServerLink.
    pub async fn approve_server_link(
        &self,
        request: crate::messages::nama::hub::v1::ApproveServerLinkRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.approve_server_link_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ApproveServerLink RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn approve_server_link_with_options(
        &self,
        request: crate::messages::nama::hub::v1::ApproveServerLinkRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ApproveServerLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINK_SERVICE_APPROVE_SERVER_LINK_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListLinkedServers RPC. Sends a request to /nama.hub.v1.LinkService/ListLinkedServers.
    pub async fn list_linked_servers(
        &self,
        request: crate::messages::nama::hub::v1::ListLinkedServersRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_linked_servers_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListLinkedServers RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_linked_servers_with_options(
        &self,
        request: crate::messages::nama::hub::v1::ListLinkedServersRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ListLinkedServersResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINK_SERVICE_LIST_LINKED_SERVERS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ApproveUserLink RPC. Sends a request to /nama.hub.v1.LinkService/ApproveUserLink.
    pub async fn approve_user_link(
        &self,
        request: crate::messages::nama::hub::v1::ApproveUserLinkRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.approve_user_link_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ApproveUserLink RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn approve_user_link_with_options(
        &self,
        request: crate::messages::nama::hub::v1::ApproveUserLinkRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ApproveUserLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINK_SERVICE_APPROVE_USER_LINK_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListLinkedUsers RPC. Sends a request to /nama.hub.v1.LinkService/ListLinkedUsers.
    pub async fn list_linked_users(
        &self,
        request: crate::messages::nama::hub::v1::ListLinkedUsersRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_linked_users_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListLinkedUsers RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_linked_users_with_options(
        &self,
        request: crate::messages::nama::hub::v1::ListLinkedUsersRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::ListLinkedUsersResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINK_SERVICE_LIST_LINKED_USERS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UnlinkLinkedUser RPC. Sends a request to /nama.hub.v1.LinkService/UnlinkLinkedUser.
    pub async fn unlink_linked_user(
        &self,
        request: crate::messages::nama::hub::v1::UnlinkLinkedUserRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.unlink_linked_user_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UnlinkLinkedUser RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn unlink_linked_user_with_options(
        &self,
        request: crate::messages::nama::hub::v1::UnlinkLinkedUserRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::UnlinkLinkedUserResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINK_SERVICE_UNLINK_LINKED_USER_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
/// Full service name for this service.
pub const LINKED_SERVER_SERVICE_SERVICE_NAME: &str = "nama.hub.v1.LinkedServerService";
/// Static [`Spec`](::connectrpc::Spec) for the `UnlinkServer` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINKED_SERVER_SERVICE_UNLINK_SERVER_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkedServerService/UnlinkServer",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `StartUserLink` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINKED_SERVER_SERVICE_START_USER_LINK_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkedServerService/StartUserLink",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `GetUserLink` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINKED_SERVER_SERVICE_GET_USER_LINK_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkedServerService/GetUserLink",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `UnlinkUser` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINKED_SERVER_SERVICE_UNLINK_USER_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkedServerService/UnlinkUser",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `PushWatchStates` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LINKED_SERVER_SERVICE_PUSH_WATCH_STATES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.hub.v1.LinkedServerService/PushWatchStates",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// What a Linked Server asks the Hub. It sends the credential `PollServerLink` gave it in an
/// `Authorization: Bearer <credential>` header, and every call fails with
/// `REFUSAL_REASON_SERVER_NOT_LINKED` without one the Hub knows.
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
pub trait LinkedServerService: Send + Sync + 'static {
    /// Unlinks the calling Server: its credential stops working at once, and it leaves its Account's
    /// Servers.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn unlink_server<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::UnlinkServerRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::UnlinkServerResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Starts Linking one of the Server's Users: the code they enter on the Hub, which replaces any
    /// code they were waiting on and stops working after `expires_in_seconds`. Fails with
    /// `REFUSAL_REASON_USER_LINKED` for a User Linked already, since a User Links to one Account.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn start_user_link<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::StartUserLinkRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::StartUserLinkResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns the Account one of the Server's Users is Linked to, if any. The Server asks while the
    /// User waits for their code to be entered, and to see whether the Account Unlinked them.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_user_link<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::GetUserLinkRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::GetUserLinkResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Unlinks one of the Server's Users, or forgets the code they wait on. Unlinking a User who
    /// isn't Linked does nothing.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn unlink_user<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::UnlinkUserRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::UnlinkUserResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Merges changes to the Server's Users' Watch State into their Accounts' (ADR 0008): per Account
    /// and title, the change made last wins, whichever Server sends it and whenever. A change of a User
    /// who isn't Linked is dropped. Fails with `REFUSAL_REASON_WATCH_STATE_INVALID` for a change it
    /// can't keep, keeping none of the call's.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn push_watch_states<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::hub::v1::PushWatchStatesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::hub::v1::PushWatchStatesResponse,
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
pub trait LinkedServerServiceExt: LinkedServerService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: LinkedServerService> LinkedServerServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                LINKED_SERVER_SERVICE_SERVICE_NAME,
                "UnlinkServer",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::UnlinkServerRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::UnlinkServerRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.unlink_server(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::UnlinkServerResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINKED_SERVER_SERVICE_UNLINK_SERVER_SPEC)
            .route_view(
                LINKED_SERVER_SERVICE_SERVICE_NAME,
                "StartUserLink",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::StartUserLinkRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::StartUserLinkRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.start_user_link(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::StartUserLinkResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINKED_SERVER_SERVICE_START_USER_LINK_SPEC)
            .route_view_idempotent(
                LINKED_SERVER_SERVICE_SERVICE_NAME,
                "GetUserLink",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::GetUserLinkRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::GetUserLinkRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_user_link(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::GetUserLinkResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINKED_SERVER_SERVICE_GET_USER_LINK_SPEC)
            .route_view(
                LINKED_SERVER_SERVICE_SERVICE_NAME,
                "UnlinkUser",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::UnlinkUserRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::UnlinkUserRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.unlink_user(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::UnlinkUserResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINKED_SERVER_SERVICE_UNLINK_USER_SPEC)
            .route_view(
                LINKED_SERVER_SERVICE_SERVICE_NAME,
                "PushWatchStates",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::hub::v1::PushWatchStatesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.push_watch_states(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::hub::v1::PushWatchStatesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LINKED_SERVER_SERVICE_PUSH_WATCH_STATES_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct LinkedServerServiceRegisterMarker;
impl<
    S: LinkedServerService,
> ::connectrpc::ServiceRegister<LinkedServerServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as LinkedServerServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `LinkedServerService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = LinkedServerServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct LinkedServerServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: LinkedServerService> LinkedServerServiceServer<T> {
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
impl<T> Clone for LinkedServerServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: LinkedServerService> ::connectrpc::Dispatcher for LinkedServerServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.hub.v1.LinkedServerService/")?;
        match method {
            "UnlinkServer" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LINKED_SERVER_SERVICE_UNLINK_SERVER_SPEC),
                )
            }
            "StartUserLink" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LINKED_SERVER_SERVICE_START_USER_LINK_SPEC),
                )
            }
            "GetUserLink" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(LINKED_SERVER_SERVICE_GET_USER_LINK_SPEC),
                )
            }
            "UnlinkUser" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LINKED_SERVER_SERVICE_UNLINK_USER_SPEC),
                )
            }
            "PushWatchStates" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LINKED_SERVER_SERVICE_PUSH_WATCH_STATES_SPEC),
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkedServerService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "UnlinkServer" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::UnlinkServerRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::UnlinkServerRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::UnlinkServerRequest,
                    >::from_parts(&req, &body);
                    svc.unlink_server(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::UnlinkServerResponse,
                        >(format)
                })
            }
            "StartUserLink" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::StartUserLinkRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::StartUserLinkRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::StartUserLinkRequest,
                    >::from_parts(&req, &body);
                    svc.start_user_link(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::StartUserLinkResponse,
                        >(format)
                })
            }
            "GetUserLink" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::GetUserLinkRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::GetUserLinkRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::GetUserLinkRequest,
                    >::from_parts(&req, &body);
                    svc.get_user_link(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::GetUserLinkResponse,
                        >(format)
                })
            }
            "UnlinkUser" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::UnlinkUserRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::UnlinkUserRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::UnlinkUserRequest,
                    >::from_parts(&req, &body);
                    svc.unlink_user(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::UnlinkUserResponse,
                        >(format)
                })
            }
            "PushWatchStates" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::hub::v1::PushWatchStatesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::hub::v1::PushWatchStatesRequest,
                    >::from_parts(&req, &body);
                    svc.push_watch_states(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::hub::v1::PushWatchStatesResponse,
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkedServerService/") else {
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkedServerService/") else {
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
        let Some(method) = path.strip_prefix("nama.hub.v1.LinkedServerService/") else {
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
/// let client = LinkedServerServiceClient::new(conn, config);
/// let response = client.unlink_server(request).await?;
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
/// let client = LinkedServerServiceClient::new(http, config);
/// let response = client.unlink_server(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.unlink_server(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.unlink_server(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct LinkedServerServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> LinkedServerServiceClient<T>
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
    /// Call the UnlinkServer RPC. Sends a request to /nama.hub.v1.LinkedServerService/UnlinkServer.
    pub async fn unlink_server(
        &self,
        request: crate::messages::nama::hub::v1::UnlinkServerRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::UnlinkServerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.unlink_server_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UnlinkServer RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn unlink_server_with_options(
        &self,
        request: crate::messages::nama::hub::v1::UnlinkServerRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::UnlinkServerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINKED_SERVER_SERVICE_UNLINK_SERVER_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the StartUserLink RPC. Sends a request to /nama.hub.v1.LinkedServerService/StartUserLink.
    pub async fn start_user_link(
        &self,
        request: crate::messages::nama::hub::v1::StartUserLinkRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::StartUserLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.start_user_link_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the StartUserLink RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn start_user_link_with_options(
        &self,
        request: crate::messages::nama::hub::v1::StartUserLinkRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::StartUserLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINKED_SERVER_SERVICE_START_USER_LINK_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetUserLink RPC. Sends a request to /nama.hub.v1.LinkedServerService/GetUserLink.
    pub async fn get_user_link(
        &self,
        request: crate::messages::nama::hub::v1::GetUserLinkRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::GetUserLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_user_link_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetUserLink RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_user_link_with_options(
        &self,
        request: crate::messages::nama::hub::v1::GetUserLinkRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::GetUserLinkResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINKED_SERVER_SERVICE_GET_USER_LINK_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UnlinkUser RPC. Sends a request to /nama.hub.v1.LinkedServerService/UnlinkUser.
    pub async fn unlink_user(
        &self,
        request: crate::messages::nama::hub::v1::UnlinkUserRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::UnlinkUserResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.unlink_user_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UnlinkUser RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn unlink_user_with_options(
        &self,
        request: crate::messages::nama::hub::v1::UnlinkUserRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::UnlinkUserResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINKED_SERVER_SERVICE_UNLINK_USER_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the PushWatchStates RPC. Sends a request to /nama.hub.v1.LinkedServerService/PushWatchStates.
    pub async fn push_watch_states(
        &self,
        request: crate::messages::nama::hub::v1::PushWatchStatesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.push_watch_states_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the PushWatchStates RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn push_watch_states_with_options(
        &self,
        request: crate::messages::nama::hub::v1::PushWatchStatesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::hub::v1::__buffa::view::PushWatchStatesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LINKED_SERVER_SERVICE_PUSH_WATCH_STATES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
