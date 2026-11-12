///Shorthand for `OwnedView<CreateLibraryRequestView<'static>>`.
pub type OwnedCreateLibraryRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateLibraryRequestView<'static>,
>;
///Shorthand for `OwnedView<CreateLibraryResponseView<'static>>`.
pub type OwnedCreateLibraryResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateLibraryResponseView<'static>,
>;
///Shorthand for `OwnedView<ListLibrariesRequestView<'static>>`.
pub type OwnedListLibrariesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListLibrariesRequestView<'static>,
>;
///Shorthand for `OwnedView<ListLibrariesResponseView<'static>>`.
pub type OwnedListLibrariesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListLibrariesResponseView<'static>,
>;
///Shorthand for `OwnedView<GetLibraryRequestView<'static>>`.
pub type OwnedGetLibraryRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetLibraryRequestView<'static>,
>;
///Shorthand for `OwnedView<GetLibraryResponseView<'static>>`.
pub type OwnedGetLibraryResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetLibraryResponseView<'static>,
>;
///Shorthand for `OwnedView<UpdateLibraryRequestView<'static>>`.
pub type OwnedUpdateLibraryRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateLibraryRequestView<'static>,
>;
///Shorthand for `OwnedView<UpdateLibraryResponseView<'static>>`.
pub type OwnedUpdateLibraryResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateLibraryResponseView<'static>,
>;
///Shorthand for `OwnedView<DeleteLibraryRequestView<'static>>`.
pub type OwnedDeleteLibraryRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteLibraryRequestView<'static>,
>;
///Shorthand for `OwnedView<DeleteLibraryResponseView<'static>>`.
pub type OwnedDeleteLibraryResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteLibraryResponseView<'static>,
>;
///Shorthand for `OwnedView<ScanLibraryRequestView<'static>>`.
pub type OwnedScanLibraryRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ScanLibraryRequestView<'static>,
>;
///Shorthand for `OwnedView<ScanLibraryResponseView<'static>>`.
pub type OwnedScanLibraryResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ScanLibraryResponseView<'static>,
>;
///Shorthand for `OwnedView<PreviewDeleteMediaRequestView<'static>>`.
pub type OwnedPreviewDeleteMediaRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaRequestView<'static>,
>;
///Shorthand for `OwnedView<PreviewDeleteMediaResponseView<'static>>`.
pub type OwnedPreviewDeleteMediaResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaResponseView<'static>,
>;
///Shorthand for `OwnedView<DeleteMediaRequestView<'static>>`.
pub type OwnedDeleteMediaRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteMediaRequestView<'static>,
>;
///Shorthand for `OwnedView<DeleteMediaResponseView<'static>>`.
pub type OwnedDeleteMediaResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteMediaResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateLibraryResponse>
for crate::messages::nama::v1::__buffa::view::CreateLibraryResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::CreateLibraryResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::CreateLibraryResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListLibrariesResponse>
for crate::messages::nama::v1::__buffa::view::ListLibrariesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListLibrariesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListLibrariesResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetLibraryResponse>
for crate::messages::nama::v1::__buffa::view::GetLibraryResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetLibraryResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetLibraryResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdateLibraryResponse>
for crate::messages::nama::v1::__buffa::view::UpdateLibraryResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::UpdateLibraryResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::UpdateLibraryResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeleteLibraryResponse>
for crate::messages::nama::v1::__buffa::view::DeleteLibraryResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeleteLibraryResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteLibraryResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ScanLibraryResponse>
for crate::messages::nama::v1::__buffa::view::ScanLibraryResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ScanLibraryResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ScanLibraryResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::PreviewDeleteMediaResponse>
for crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::PreviewDeleteMediaResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeleteMediaResponse>
for crate::messages::nama::v1::__buffa::view::DeleteMediaResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::DeleteMediaResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::DeleteMediaResponseView<'static>,
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
pub const LIBRARY_SERVICE_SERVICE_NAME: &str = "nama.v1.LibraryService";
/// Static [`Spec`](::connectrpc::Spec) for the `CreateLibrary` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_CREATE_LIBRARY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/CreateLibrary",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListLibraries` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_LIST_LIBRARIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/ListLibraries",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetLibrary` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_GET_LIBRARY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/GetLibrary",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `UpdateLibrary` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_UPDATE_LIBRARY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/UpdateLibrary",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `DeleteLibrary` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_DELETE_LIBRARY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/DeleteLibrary",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `ScanLibrary` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_SCAN_LIBRARY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/ScanLibrary",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `PreviewDeleteMedia` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_PREVIEW_DELETE_MEDIA_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/PreviewDeleteMedia",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `DeleteMedia` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LIBRARY_SERVICE_DELETE_MEDIA_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.LibraryService/DeleteMedia",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// The Server's Libraries: named collections of one kind of media, each backed by one or more
/// folders on the Server's disk. Every RPC but `ListLibraries` and those deleting media needs
/// `PERMISSION_MANAGE_LIBRARIES`, and fails with `permission_denied` without it.
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
pub trait LibraryService: Send + Sync + 'static {
    /// Creates a Library. Fails with `invalid_argument` if the name is blank, the kind is
    /// unspecified, or a folder isn't an absolute path to a readable directory, and with
    /// `already_exists` if a folder already belongs to another Library.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_library<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::CreateLibraryRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::CreateLibraryResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists every Library on the Server, oldest first. Any signed-in User may list them, to browse,
    /// but only one with `PERMISSION_MANAGE_LIBRARIES` gets their folders.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_libraries<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListLibrariesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListLibrariesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns one Library, or fails with `not_found`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_library<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetLibraryRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetLibraryResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Replaces a Library's name and folders. Its kind can't change. Fails like `CreateLibrary`,
    /// or with `not_found` if there is no such Library.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn update_library<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::UpdateLibraryRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::UpdateLibraryResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Deletes a Library. The files in its folders are left alone. Fails with `not_found` if there
    /// is no such Library, so a retry after a delete that went through gets `not_found`.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn delete_library<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::DeleteLibraryRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::DeleteLibraryResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Starts scanning a Library's folders for video files, and returns without waiting for the
    /// scan to finish. If the Library is already being scanned, one more scan follows it, so
    /// changes made since the running scan began are picked up. Fails with `not_found` if there is
    /// no such Library.
    ///
    /// To wait for the scan, subscribe to events about the Library first, then call this, then skip
    /// to the next `ScanStarted`: the first `MatchingFinished` and `ProbingFinished` after it cover
    /// this call.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn scan_library<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ScanLibraryRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ScanLibraryResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists the files `DeleteMedia` would delete for the same target, and the bytes they would
    /// free, deleting nothing. Needs `PERMISSION_DELETE_MEDIA`. Fails as `DeleteMedia` does, except
    /// that a read-only Library folder is named in `read_only_folder` rather than refused.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn preview_delete_media<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::PreviewDeleteMediaRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::PreviewDeleteMediaResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Deletes media from disk for good, with nothing to undo it: each Version's file, its sidecars
    /// (the files beside it whose names start with the video's name, without its extension, followed
    /// by `.` or `-`, and that aren't videos), and the folders that leaves empty, never a Library's
    /// own folder or above it. The Versions whose files went are removed as a scan removes them, with
    /// `LibraryItemRemoved`. A file that can't be deleted is listed with why, and the rest still go.
    /// Before deleting anything, it asks every enabled Fulfiller of its kind, such as Radarr, to stop
    /// monitoring the title, which it keeps; one that can't be told is listed, and doesn't stop it.
    /// The Playback Plans playing the Versions end next, each with `PlaybackEnded`, and a Version
    /// being served for Direct Play is cut off. Last, the open Requests for the Movie or each Season
    /// it took anything of are cancelled, each with `RequestCancelled` naming who deleted it. Needs `PERMISSION_DELETE_MEDIA` and Recent
    /// Authentication. Fails with `invalid_argument` if the target is missing, or names a Library of
    /// the wrong kind, with `not_found` if there is no such Library, title, Season or Version, and
    /// with `failed_precondition` if a file it would delete is in a Library folder the Server
    /// couldn't write to when it last scanned the Library.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn delete_media<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::DeleteMediaRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::DeleteMediaResponse,
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
pub trait LibraryServiceExt: LibraryService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: LibraryService> LibraryServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                LIBRARY_SERVICE_SERVICE_NAME,
                "CreateLibrary",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::CreateLibraryRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::CreateLibraryRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_library(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::CreateLibraryResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_CREATE_LIBRARY_SPEC)
            .route_view_idempotent(
                LIBRARY_SERVICE_SERVICE_NAME,
                "ListLibraries",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListLibrariesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListLibrariesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_libraries(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListLibrariesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_LIST_LIBRARIES_SPEC)
            .route_view_idempotent(
                LIBRARY_SERVICE_SERVICE_NAME,
                "GetLibrary",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetLibraryRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetLibraryRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_library(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetLibraryResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_GET_LIBRARY_SPEC)
            .route_view(
                LIBRARY_SERVICE_SERVICE_NAME,
                "UpdateLibrary",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::UpdateLibraryRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::UpdateLibraryRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.update_library(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::UpdateLibraryResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_UPDATE_LIBRARY_SPEC)
            .route_view(
                LIBRARY_SERVICE_SERVICE_NAME,
                "DeleteLibrary",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::DeleteLibraryRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::DeleteLibraryRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.delete_library(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::DeleteLibraryResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_DELETE_LIBRARY_SPEC)
            .route_view(
                LIBRARY_SERVICE_SERVICE_NAME,
                "ScanLibrary",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ScanLibraryRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ScanLibraryRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.scan_library(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ScanLibraryResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_SCAN_LIBRARY_SPEC)
            .route_view_idempotent(
                LIBRARY_SERVICE_SERVICE_NAME,
                "PreviewDeleteMedia",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::PreviewDeleteMediaRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.preview_delete_media(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::PreviewDeleteMediaResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_PREVIEW_DELETE_MEDIA_SPEC)
            .route_view(
                LIBRARY_SERVICE_SERVICE_NAME,
                "DeleteMedia",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::DeleteMediaRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::DeleteMediaRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.delete_media(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::DeleteMediaResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LIBRARY_SERVICE_DELETE_MEDIA_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct LibraryServiceRegisterMarker;
impl<S: LibraryService> ::connectrpc::ServiceRegister<LibraryServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as LibraryServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `LibraryService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = LibraryServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct LibraryServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: LibraryService> LibraryServiceServer<T> {
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
impl<T> Clone for LibraryServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: LibraryService> ::connectrpc::Dispatcher for LibraryServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.LibraryService/")?;
        match method {
            "CreateLibrary" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LIBRARY_SERVICE_CREATE_LIBRARY_SPEC),
                )
            }
            "ListLibraries" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(LIBRARY_SERVICE_LIST_LIBRARIES_SPEC),
                )
            }
            "GetLibrary" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(LIBRARY_SERVICE_GET_LIBRARY_SPEC),
                )
            }
            "UpdateLibrary" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LIBRARY_SERVICE_UPDATE_LIBRARY_SPEC),
                )
            }
            "DeleteLibrary" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LIBRARY_SERVICE_DELETE_LIBRARY_SPEC),
                )
            }
            "ScanLibrary" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LIBRARY_SERVICE_SCAN_LIBRARY_SPEC),
                )
            }
            "PreviewDeleteMedia" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(LIBRARY_SERVICE_PREVIEW_DELETE_MEDIA_SPEC),
                )
            }
            "DeleteMedia" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(LIBRARY_SERVICE_DELETE_MEDIA_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.LibraryService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "CreateLibrary" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::CreateLibraryRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::CreateLibraryRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::CreateLibraryRequest,
                    >::from_parts(&req, &body);
                    svc.create_library(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::CreateLibraryResponse,
                        >(format)
                })
            }
            "ListLibraries" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListLibrariesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListLibrariesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListLibrariesRequest,
                    >::from_parts(&req, &body);
                    svc.list_libraries(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ListLibrariesResponse,
                        >(format)
                })
            }
            "GetLibrary" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetLibraryRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetLibraryRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetLibraryRequest,
                    >::from_parts(&req, &body);
                    svc.get_library(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::GetLibraryResponse>(format)
                })
            }
            "UpdateLibrary" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::UpdateLibraryRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::UpdateLibraryRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::UpdateLibraryRequest,
                    >::from_parts(&req, &body);
                    svc.update_library(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::UpdateLibraryResponse,
                        >(format)
                })
            }
            "DeleteLibrary" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::DeleteLibraryRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::DeleteLibraryRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::DeleteLibraryRequest,
                    >::from_parts(&req, &body);
                    svc.delete_library(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::DeleteLibraryResponse,
                        >(format)
                })
            }
            "ScanLibrary" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ScanLibraryRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ScanLibraryRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ScanLibraryRequest,
                    >::from_parts(&req, &body);
                    svc.scan_library(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ScanLibraryResponse>(format)
                })
            }
            "PreviewDeleteMedia" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::PreviewDeleteMediaRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::PreviewDeleteMediaRequest,
                    >::from_parts(&req, &body);
                    svc.preview_delete_media(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::PreviewDeleteMediaResponse,
                        >(format)
                })
            }
            "DeleteMedia" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::DeleteMediaRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::DeleteMediaRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::DeleteMediaRequest,
                    >::from_parts(&req, &body);
                    svc.delete_media(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::DeleteMediaResponse>(format)
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
        let Some(method) = path.strip_prefix("nama.v1.LibraryService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.LibraryService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.LibraryService/") else {
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
/// let client = LibraryServiceClient::new(conn, config);
/// let response = client.create_library(request).await?;
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
/// let client = LibraryServiceClient::new(http, config);
/// let response = client.create_library(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.create_library(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.create_library(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct LibraryServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> LibraryServiceClient<T>
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
    /// Call the CreateLibrary RPC. Sends a request to /nama.v1.LibraryService/CreateLibrary.
    pub async fn create_library(
        &self,
        request: crate::messages::nama::v1::CreateLibraryRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_library_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateLibrary RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_library_with_options(
        &self,
        request: crate::messages::nama::v1::CreateLibraryRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::CreateLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_CREATE_LIBRARY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListLibraries RPC. Sends a request to /nama.v1.LibraryService/ListLibraries.
    pub async fn list_libraries(
        &self,
        request: crate::messages::nama::v1::ListLibrariesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListLibrariesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_libraries_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListLibraries RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_libraries_with_options(
        &self,
        request: crate::messages::nama::v1::ListLibrariesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListLibrariesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_LIST_LIBRARIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetLibrary RPC. Sends a request to /nama.v1.LibraryService/GetLibrary.
    pub async fn get_library(
        &self,
        request: crate::messages::nama::v1::GetLibraryRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetLibraryResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_library_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetLibrary RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_library_with_options(
        &self,
        request: crate::messages::nama::v1::GetLibraryRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetLibraryResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_GET_LIBRARY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UpdateLibrary RPC. Sends a request to /nama.v1.LibraryService/UpdateLibrary.
    pub async fn update_library(
        &self,
        request: crate::messages::nama::v1::UpdateLibraryRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdateLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.update_library_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UpdateLibrary RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn update_library_with_options(
        &self,
        request: crate::messages::nama::v1::UpdateLibraryRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::UpdateLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_UPDATE_LIBRARY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the DeleteLibrary RPC. Sends a request to /nama.v1.LibraryService/DeleteLibrary.
    pub async fn delete_library(
        &self,
        request: crate::messages::nama::v1::DeleteLibraryRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeleteLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.delete_library_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the DeleteLibrary RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn delete_library_with_options(
        &self,
        request: crate::messages::nama::v1::DeleteLibraryRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeleteLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_DELETE_LIBRARY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ScanLibrary RPC. Sends a request to /nama.v1.LibraryService/ScanLibrary.
    pub async fn scan_library(
        &self,
        request: crate::messages::nama::v1::ScanLibraryRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ScanLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.scan_library_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ScanLibrary RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn scan_library_with_options(
        &self,
        request: crate::messages::nama::v1::ScanLibraryRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ScanLibraryResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_SCAN_LIBRARY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the PreviewDeleteMedia RPC. Sends a request to /nama.v1.LibraryService/PreviewDeleteMedia.
    pub async fn preview_delete_media(
        &self,
        request: crate::messages::nama::v1::PreviewDeleteMediaRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.preview_delete_media_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the PreviewDeleteMedia RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn preview_delete_media_with_options(
        &self,
        request: crate::messages::nama::v1::PreviewDeleteMediaRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::PreviewDeleteMediaResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_PREVIEW_DELETE_MEDIA_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the DeleteMedia RPC. Sends a request to /nama.v1.LibraryService/DeleteMedia.
    pub async fn delete_media(
        &self,
        request: crate::messages::nama::v1::DeleteMediaRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeleteMediaResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.delete_media_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the DeleteMedia RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn delete_media_with_options(
        &self,
        request: crate::messages::nama::v1::DeleteMediaRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::DeleteMediaResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                LIBRARY_SERVICE_DELETE_MEDIA_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
