///Shorthand for `OwnedView<CreateRequestRequestView<'static>>`.
pub type OwnedCreateRequestRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateRequestRequestView<'static>,
>;
///Shorthand for `OwnedView<CreateRequestResponseView<'static>>`.
pub type OwnedCreateRequestResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateRequestResponseView<'static>,
>;
///Shorthand for `OwnedView<CreateSeasonRequestsRequestView<'static>>`.
pub type OwnedCreateSeasonRequestsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsRequestView<'static>,
>;
///Shorthand for `OwnedView<CreateSeasonRequestsResponseView<'static>>`.
pub type OwnedCreateSeasonRequestsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsResponseView<'static>,
>;
///Shorthand for `OwnedView<ListRequestsRequestView<'static>>`.
pub type OwnedListRequestsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListRequestsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListRequestsResponseView<'static>>`.
pub type OwnedListRequestsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListRequestsResponseView<'static>,
>;
///Shorthand for `OwnedView<GetRequestRequestView<'static>>`.
pub type OwnedGetRequestRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetRequestRequestView<'static>,
>;
///Shorthand for `OwnedView<GetRequestResponseView<'static>>`.
pub type OwnedGetRequestResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetRequestResponseView<'static>,
>;
///Shorthand for `OwnedView<SearchMoviesRequestView<'static>>`.
pub type OwnedSearchMoviesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SearchMoviesRequestView<'static>,
>;
///Shorthand for `OwnedView<SearchMoviesResponseView<'static>>`.
pub type OwnedSearchMoviesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SearchMoviesResponseView<'static>,
>;
///Shorthand for `OwnedView<LookUpMovieRequestView<'static>>`.
pub type OwnedLookUpMovieRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::LookUpMovieRequestView<'static>,
>;
///Shorthand for `OwnedView<LookUpMovieResponseView<'static>>`.
pub type OwnedLookUpMovieResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::LookUpMovieResponseView<'static>,
>;
///Shorthand for `OwnedView<ApproveRequestRequestView<'static>>`.
pub type OwnedApproveRequestRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ApproveRequestRequestView<'static>,
>;
///Shorthand for `OwnedView<ApproveRequestResponseView<'static>>`.
pub type OwnedApproveRequestResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ApproveRequestResponseView<'static>,
>;
///Shorthand for `OwnedView<DeclineRequestRequestView<'static>>`.
pub type OwnedDeclineRequestRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeclineRequestRequestView<'static>,
>;
///Shorthand for `OwnedView<DeclineRequestResponseView<'static>>`.
pub type OwnedDeclineRequestResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeclineRequestResponseView<'static>,
>;
///Shorthand for `OwnedView<CancelRequestRequestView<'static>>`.
pub type OwnedCancelRequestRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CancelRequestRequestView<'static>,
>;
///Shorthand for `OwnedView<CancelRequestResponseView<'static>>`.
pub type OwnedCancelRequestResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CancelRequestResponseView<'static>,
>;
///Shorthand for `OwnedView<WithdrawRequestRequestView<'static>>`.
pub type OwnedWithdrawRequestRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::WithdrawRequestRequestView<'static>,
>;
///Shorthand for `OwnedView<WithdrawRequestResponseView<'static>>`.
pub type OwnedWithdrawRequestResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::WithdrawRequestResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateRequestResponse>
for crate::messages::nama::v1::__buffa::view::CreateRequestResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateRequestResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateRequestResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateSeasonRequestsResponse>
for crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateSeasonRequestsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListRequestsResponse>
for crate::messages::nama::v1::__buffa::view::ListRequestsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListRequestsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListRequestsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetRequestResponse>
for crate::messages::nama::v1::__buffa::view::GetRequestResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetRequestResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetRequestResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::SearchMoviesResponse>
for crate::messages::nama::v1::__buffa::view::SearchMoviesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::SearchMoviesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SearchMoviesResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::LookUpMovieResponse>
for crate::messages::nama::v1::__buffa::view::LookUpMovieResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::LookUpMovieResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::LookUpMovieResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ApproveRequestResponse>
for crate::messages::nama::v1::__buffa::view::ApproveRequestResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ApproveRequestResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ApproveRequestResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeclineRequestResponse>
for crate::messages::nama::v1::__buffa::view::DeclineRequestResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeclineRequestResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeclineRequestResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::CancelRequestResponse>
for crate::messages::nama::v1::__buffa::view::CancelRequestResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::CancelRequestResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CancelRequestResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::WithdrawRequestResponse>
for crate::messages::nama::v1::__buffa::view::WithdrawRequestResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::WithdrawRequestResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::WithdrawRequestResponseView<'static>,
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
pub const REQUEST_SERVICE_SERVICE_NAME: &str = "nama.v1.RequestService";
/// Static [`Spec`](::connectrpc::Spec) for the `CreateRequest` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_CREATE_REQUEST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/CreateRequest",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `CreateSeasonRequests` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_CREATE_SEASON_REQUESTS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/CreateSeasonRequests",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListRequests` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_LIST_REQUESTS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/ListRequests",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetRequest` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_GET_REQUEST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/GetRequest",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `SearchMovies` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_SEARCH_MOVIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/SearchMovies",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `LookUpMovie` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_LOOK_UP_MOVIE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/LookUpMovie",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ApproveRequest` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_APPROVE_REQUEST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/ApproveRequest",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `DeclineRequest` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_DECLINE_REQUEST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/DeclineRequest",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `CancelRequest` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_CANCEL_REQUEST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/CancelRequest",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `WithdrawRequest` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const REQUEST_SERVICE_WITHDRAW_REQUEST_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.RequestService/WithdrawRequest",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Users asking for Movies, and Seasons of Shows, that aren't on the Server to be added to it
/// (ADR 0012). Without `PERMISSION_VIEW_REQUESTS`, a User sees only the Requests they are a
/// Requester of, here and in `EventService`.
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
pub trait RequestService: Send + Sync + 'static {
    /// Asks for a Movie for the caller. If the Movie has an open Request, the caller joins it as a
    /// Requester, which changes nothing if they already are one. Otherwise a new Request opens:
    /// Approved if the caller has `PERMISSION_AUTO_APPROVE_MOVIE`, else Pending. Needs
    /// `PERMISSION_REQUEST_MOVIE`. Fails with `failed_precondition` if a Library already has the
    /// Movie, `invalid_argument` if the metadata source has no Movie with the id, and `unavailable`
    /// if no metadata Provider is enabled, or it failed.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_request<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::CreateRequestRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::CreateRequestResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Asks for Seasons of a Show for the caller, one Request per Season, each as `CreateRequest`
    /// asks for a Movie: Approved if the caller has `PERMISSION_AUTO_APPROVE_TV`, else Pending. Needs
    /// `PERMISSION_REQUEST_TV`. A Season is in a Library once it has aired and every one of its aired
    /// Episodes is Matched, and asking for such a Season fails with `failed_precondition`. Fails with
    /// `invalid_argument` if no Season is named or the metadata source has no such Show or Season,
    /// and `unavailable` if no metadata Provider is enabled, or it failed. A failure changes no
    /// Request.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_season_requests<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::CreateSeasonRequestsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::CreateSeasonRequestsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists the Requests the caller may see, newest first.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_requests<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListRequestsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListRequestsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns one Request. Fails with `not_found` if there is none with the ID, or none the caller
    /// may see, so they can't learn which IDs exist.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_request<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetRequestRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetRequestResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Searches the metadata source for Movies to ask for, best fit first. Unlike
    /// `MatchService.SearchMetadata`, it needs only `PERMISSION_REQUEST_MOVIE`. Fails with
    /// `invalid_argument` if the title is blank, and `unavailable` if no metadata Provider is
    /// enabled, or it failed.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn search_movies<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::SearchMoviesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::SearchMoviesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns one Movie from the metadata source, for its page when no Library has it. Needs
    /// `PERMISSION_REQUEST_MOVIE`. Fails with `invalid_argument` if the metadata source has no Movie
    /// with the id, and `unavailable` if no metadata Provider is enabled, or it failed.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn look_up_movie<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::LookUpMovieRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::LookUpMovieResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Approves a Pending Request, which then waits for the Movie to be Matched in a Library. Needs
    /// `PERMISSION_MANAGE_REQUESTS`, which manages every User's Requests. Fails with `not_found` if
    /// there is no Request with the ID, and `failed_precondition` if it isn't Pending.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn approve_request<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ApproveRequestRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ApproveRequestResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Declines a Pending Request, which closes it: asking for the Movie again opens a new one. Needs
    /// `PERMISSION_MANAGE_REQUESTS`. Fails as `ApproveRequest` does.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn decline_request<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::DeclineRequestRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::DeclineRequestResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Removes a Request, whatever its state. Needs `PERMISSION_MANAGE_REQUESTS`. Fails with
    /// `not_found` if there is no Request with the ID.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn cancel_request<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::CancelRequestRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::CancelRequestResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Takes the caller off a Pending Request's Requesters, which removes the Request once nobody is
    /// left on it. Fails with `not_found` if there is no Request with the ID, or none the caller may
    /// see, and `failed_precondition` if the caller isn't one of its Requesters or it isn't Pending.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn withdraw_request<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::WithdrawRequestRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::WithdrawRequestResponse,
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
pub trait RequestServiceExt: RequestService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: RequestService> RequestServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                REQUEST_SERVICE_SERVICE_NAME,
                "CreateRequest",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::CreateRequestRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::CreateRequestRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_request(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::CreateRequestResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_CREATE_REQUEST_SPEC)
            .route_view(
                REQUEST_SERVICE_SERVICE_NAME,
                "CreateSeasonRequests",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::CreateSeasonRequestsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_season_requests(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::CreateSeasonRequestsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_CREATE_SEASON_REQUESTS_SPEC)
            .route_view_idempotent(
                REQUEST_SERVICE_SERVICE_NAME,
                "ListRequests",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListRequestsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListRequestsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_requests(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListRequestsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_LIST_REQUESTS_SPEC)
            .route_view_idempotent(
                REQUEST_SERVICE_SERVICE_NAME,
                "GetRequest",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetRequestRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetRequestRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_request(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetRequestResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_GET_REQUEST_SPEC)
            .route_view_idempotent(
                REQUEST_SERVICE_SERVICE_NAME,
                "SearchMovies",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::SearchMoviesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::SearchMoviesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.search_movies(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::SearchMoviesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_SEARCH_MOVIES_SPEC)
            .route_view_idempotent(
                REQUEST_SERVICE_SERVICE_NAME,
                "LookUpMovie",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::LookUpMovieRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::LookUpMovieRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.look_up_movie(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::LookUpMovieResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_LOOK_UP_MOVIE_SPEC)
            .route_view(
                REQUEST_SERVICE_SERVICE_NAME,
                "ApproveRequest",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ApproveRequestRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ApproveRequestRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.approve_request(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ApproveRequestResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_APPROVE_REQUEST_SPEC)
            .route_view(
                REQUEST_SERVICE_SERVICE_NAME,
                "DeclineRequest",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::DeclineRequestRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::DeclineRequestRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.decline_request(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::DeclineRequestResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_DECLINE_REQUEST_SPEC)
            .route_view(
                REQUEST_SERVICE_SERVICE_NAME,
                "CancelRequest",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::CancelRequestRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::CancelRequestRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.cancel_request(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::CancelRequestResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_CANCEL_REQUEST_SPEC)
            .route_view(
                REQUEST_SERVICE_SERVICE_NAME,
                "WithdrawRequest",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::WithdrawRequestRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::WithdrawRequestRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.withdraw_request(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::WithdrawRequestResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(REQUEST_SERVICE_WITHDRAW_REQUEST_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct RequestServiceRegisterMarker;
impl<S: RequestService> ::connectrpc::ServiceRegister<RequestServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as RequestServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `RequestService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = RequestServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct RequestServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: RequestService> RequestServiceServer<T> {
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
impl<T> Clone for RequestServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: RequestService> ::connectrpc::Dispatcher for RequestServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.RequestService/")?;
        match method {
            "CreateRequest" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(REQUEST_SERVICE_CREATE_REQUEST_SPEC),
                )
            }
            "CreateSeasonRequests" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(REQUEST_SERVICE_CREATE_SEASON_REQUESTS_SPEC),
                )
            }
            "ListRequests" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(REQUEST_SERVICE_LIST_REQUESTS_SPEC),
                )
            }
            "GetRequest" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(REQUEST_SERVICE_GET_REQUEST_SPEC),
                )
            }
            "SearchMovies" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(REQUEST_SERVICE_SEARCH_MOVIES_SPEC),
                )
            }
            "LookUpMovie" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(REQUEST_SERVICE_LOOK_UP_MOVIE_SPEC),
                )
            }
            "ApproveRequest" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(REQUEST_SERVICE_APPROVE_REQUEST_SPEC),
                )
            }
            "DeclineRequest" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(REQUEST_SERVICE_DECLINE_REQUEST_SPEC),
                )
            }
            "CancelRequest" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(REQUEST_SERVICE_CANCEL_REQUEST_SPEC),
                )
            }
            "WithdrawRequest" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(REQUEST_SERVICE_WITHDRAW_REQUEST_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.RequestService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "CreateRequest" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::CreateRequestRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::CreateRequestRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::CreateRequestRequest,
                    >::from_parts(&req, &body);
                    svc.create_request(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::CreateRequestResponse,
                        >(format)
                })
            }
            "CreateSeasonRequests" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::CreateSeasonRequestsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::CreateSeasonRequestsRequest,
                    >::from_parts(&req, &body);
                    svc.create_season_requests(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::CreateSeasonRequestsResponse,
                        >(format)
                })
            }
            "ListRequests" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListRequestsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListRequestsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListRequestsRequest,
                    >::from_parts(&req, &body);
                    svc.list_requests(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ListRequestsResponse,
                        >(format)
                })
            }
            "GetRequest" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetRequestRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetRequestRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetRequestRequest,
                    >::from_parts(&req, &body);
                    svc.get_request(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::GetRequestResponse>(format)
                })
            }
            "SearchMovies" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::SearchMoviesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::SearchMoviesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::SearchMoviesRequest,
                    >::from_parts(&req, &body);
                    svc.search_movies(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::SearchMoviesResponse,
                        >(format)
                })
            }
            "LookUpMovie" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::LookUpMovieRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::LookUpMovieRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::LookUpMovieRequest,
                    >::from_parts(&req, &body);
                    svc.look_up_movie(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::LookUpMovieResponse>(format)
                })
            }
            "ApproveRequest" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ApproveRequestRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ApproveRequestRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ApproveRequestRequest,
                    >::from_parts(&req, &body);
                    svc.approve_request(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ApproveRequestResponse,
                        >(format)
                })
            }
            "DeclineRequest" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::DeclineRequestRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::DeclineRequestRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::DeclineRequestRequest,
                    >::from_parts(&req, &body);
                    svc.decline_request(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::DeclineRequestResponse,
                        >(format)
                })
            }
            "CancelRequest" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::CancelRequestRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::CancelRequestRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::CancelRequestRequest,
                    >::from_parts(&req, &body);
                    svc.cancel_request(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::CancelRequestResponse,
                        >(format)
                })
            }
            "WithdrawRequest" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::WithdrawRequestRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::WithdrawRequestRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::WithdrawRequestRequest,
                    >::from_parts(&req, &body);
                    svc.withdraw_request(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::WithdrawRequestResponse,
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
        let Some(method) = path.strip_prefix("nama.v1.RequestService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.RequestService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.RequestService/") else {
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
/// let client = RequestServiceClient::new(conn, config);
/// let response = client.create_request(request).await?;
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
/// let client = RequestServiceClient::new(http, config);
/// let response = client.create_request(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.create_request(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.create_request(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct RequestServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> RequestServiceClient<T>
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
    /// Call the CreateRequest RPC. Sends a request to /nama.v1.RequestService/CreateRequest.
    pub async fn create_request(
        &self,
        request: crate::messages::nama::v1::CreateRequestRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_request_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateRequest RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_request_with_options(
        &self,
        request: crate::messages::nama::v1::CreateRequestRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_CREATE_REQUEST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CreateSeasonRequests RPC. Sends a request to /nama.v1.RequestService/CreateSeasonRequests.
    pub async fn create_season_requests(
        &self,
        request: crate::messages::nama::v1::CreateSeasonRequestsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_season_requests_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateSeasonRequests RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_season_requests_with_options(
        &self,
        request: crate::messages::nama::v1::CreateSeasonRequestsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateSeasonRequestsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_CREATE_SEASON_REQUESTS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListRequests RPC. Sends a request to /nama.v1.RequestService/ListRequests.
    pub async fn list_requests(
        &self,
        request: crate::messages::nama::v1::ListRequestsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListRequestsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_requests_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListRequests RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_requests_with_options(
        &self,
        request: crate::messages::nama::v1::ListRequestsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListRequestsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_LIST_REQUESTS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetRequest RPC. Sends a request to /nama.v1.RequestService/GetRequest.
    pub async fn get_request(
        &self,
        request: crate::messages::nama::v1::GetRequestRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetRequestResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_request_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetRequest RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_request_with_options(
        &self,
        request: crate::messages::nama::v1::GetRequestRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetRequestResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_GET_REQUEST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SearchMovies RPC. Sends a request to /nama.v1.RequestService/SearchMovies.
    pub async fn search_movies(
        &self,
        request: crate::messages::nama::v1::SearchMoviesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SearchMoviesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.search_movies_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SearchMovies RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn search_movies_with_options(
        &self,
        request: crate::messages::nama::v1::SearchMoviesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SearchMoviesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_SEARCH_MOVIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the LookUpMovie RPC. Sends a request to /nama.v1.RequestService/LookUpMovie.
    pub async fn look_up_movie(
        &self,
        request: crate::messages::nama::v1::LookUpMovieRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::LookUpMovieResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.look_up_movie_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the LookUpMovie RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn look_up_movie_with_options(
        &self,
        request: crate::messages::nama::v1::LookUpMovieRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::LookUpMovieResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_LOOK_UP_MOVIE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ApproveRequest RPC. Sends a request to /nama.v1.RequestService/ApproveRequest.
    pub async fn approve_request(
        &self,
        request: crate::messages::nama::v1::ApproveRequestRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ApproveRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.approve_request_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ApproveRequest RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn approve_request_with_options(
        &self,
        request: crate::messages::nama::v1::ApproveRequestRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ApproveRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_APPROVE_REQUEST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the DeclineRequest RPC. Sends a request to /nama.v1.RequestService/DeclineRequest.
    pub async fn decline_request(
        &self,
        request: crate::messages::nama::v1::DeclineRequestRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeclineRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.decline_request_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the DeclineRequest RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn decline_request_with_options(
        &self,
        request: crate::messages::nama::v1::DeclineRequestRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeclineRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_DECLINE_REQUEST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CancelRequest RPC. Sends a request to /nama.v1.RequestService/CancelRequest.
    pub async fn cancel_request(
        &self,
        request: crate::messages::nama::v1::CancelRequestRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CancelRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.cancel_request_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CancelRequest RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn cancel_request_with_options(
        &self,
        request: crate::messages::nama::v1::CancelRequestRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CancelRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_CANCEL_REQUEST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the WithdrawRequest RPC. Sends a request to /nama.v1.RequestService/WithdrawRequest.
    pub async fn withdraw_request(
        &self,
        request: crate::messages::nama::v1::WithdrawRequestRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::WithdrawRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.withdraw_request_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the WithdrawRequest RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn withdraw_request_with_options(
        &self,
        request: crate::messages::nama::v1::WithdrawRequestRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::WithdrawRequestResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                REQUEST_SERVICE_WITHDRAW_REQUEST_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
