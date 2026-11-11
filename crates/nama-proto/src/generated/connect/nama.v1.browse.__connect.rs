///Shorthand for `OwnedView<ListMoviesRequestView<'static>>`.
pub type OwnedListMoviesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListMoviesRequestView<'static>,
>;
///Shorthand for `OwnedView<ListMoviesResponseView<'static>>`.
pub type OwnedListMoviesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListMoviesResponseView<'static>,
>;
///Shorthand for `OwnedView<GetMovieRequestView<'static>>`.
pub type OwnedGetMovieRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetMovieRequestView<'static>,
>;
///Shorthand for `OwnedView<GetMovieResponseView<'static>>`.
pub type OwnedGetMovieResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetMovieResponseView<'static>,
>;
///Shorthand for `OwnedView<ListShowsRequestView<'static>>`.
pub type OwnedListShowsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListShowsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListShowsResponseView<'static>>`.
pub type OwnedListShowsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListShowsResponseView<'static>,
>;
///Shorthand for `OwnedView<GetShowRequestView<'static>>`.
pub type OwnedGetShowRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetShowRequestView<'static>,
>;
///Shorthand for `OwnedView<GetShowResponseView<'static>>`.
pub type OwnedGetShowResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetShowResponseView<'static>,
>;
///Shorthand for `OwnedView<ListEpisodesRequestView<'static>>`.
pub type OwnedListEpisodesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListEpisodesRequestView<'static>,
>;
///Shorthand for `OwnedView<ListEpisodesResponseView<'static>>`.
pub type OwnedListEpisodesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListEpisodesResponseView<'static>,
>;
///Shorthand for `OwnedView<ListContinueWatchingRequestView<'static>>`.
pub type OwnedListContinueWatchingRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListContinueWatchingRequestView<'static>,
>;
///Shorthand for `OwnedView<ListContinueWatchingResponseView<'static>>`.
pub type OwnedListContinueWatchingResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListContinueWatchingResponseView<'static>,
>;
///Shorthand for `OwnedView<SearchLibrariesRequestView<'static>>`.
pub type OwnedSearchLibrariesRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SearchLibrariesRequestView<'static>,
>;
///Shorthand for `OwnedView<SearchLibrariesResponseView<'static>>`.
pub type OwnedSearchLibrariesResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SearchLibrariesResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListMoviesResponse>
for crate::messages::nama::v1::__buffa::view::ListMoviesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListMoviesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListMoviesResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetMovieResponse>
for crate::messages::nama::v1::__buffa::view::GetMovieResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetMovieResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetMovieResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListShowsResponse>
for crate::messages::nama::v1::__buffa::view::ListShowsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListShowsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListShowsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetShowResponse>
for crate::messages::nama::v1::__buffa::view::GetShowResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::GetShowResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::GetShowResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListEpisodesResponse>
for crate::messages::nama::v1::__buffa::view::ListEpisodesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListEpisodesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListEpisodesResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListContinueWatchingResponse>
for crate::messages::nama::v1::__buffa::view::ListContinueWatchingResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListContinueWatchingResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListContinueWatchingResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::SearchLibrariesResponse>
for crate::messages::nama::v1::__buffa::view::SearchLibrariesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::SearchLibrariesResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::SearchLibrariesResponseView<'static>,
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
pub const BROWSE_SERVICE_SERVICE_NAME: &str = "nama.v1.BrowseService";
/// Static [`Spec`](::connectrpc::Spec) for the `ListMovies` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const BROWSE_SERVICE_LIST_MOVIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.BrowseService/ListMovies",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetMovie` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const BROWSE_SERVICE_GET_MOVIE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.BrowseService/GetMovie",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ListShows` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const BROWSE_SERVICE_LIST_SHOWS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.BrowseService/ListShows",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetShow` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const BROWSE_SERVICE_GET_SHOW_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.BrowseService/GetShow",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ListEpisodes` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const BROWSE_SERVICE_LIST_EPISODES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.BrowseService/ListEpisodes",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ListContinueWatching` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const BROWSE_SERVICE_LIST_CONTINUE_WATCHING_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.BrowseService/ListContinueWatching",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `SearchLibraries` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const BROWSE_SERVICE_SEARCH_LIBRARIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.BrowseService/SearchLibraries",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Browsing what a Library holds, and the caller's Continue Watching, for any signed-in User.
///
/// Only Matched items appear. Movies, Shows, Seasons and Episodes are not stored as such: they are
/// grouped from the items Matched to the same title, and identified by its external id within the
/// Library. A Show is grouped from its Episodes' Show ids, and a Season is the Show's Episodes that
/// share a season number. Items that aren't Matched are in `MatchService.ListUnmatchedItems`
/// instead.
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
pub trait BrowseService: Send + Sync + 'static {
    /// Lists a Movies Library's Movies, by title, each with its Versions and the caller's Watch
    /// State. Fails with `not_found` if there is no such Library, and with `invalid_argument` if it
    /// is a Shows Library.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_movies<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListMoviesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListMoviesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns one Movie of a Movies Library, with its Versions and the caller's Watch State. Fails
    /// like `ListMovies`, or with `not_found` if nothing in the Library is Matched to that id.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_movie<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetMovieRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetMovieResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists a Shows Library's Shows, by title. Fails with `not_found` if there is no such Library,
    /// and with `invalid_argument` if it is a Movies Library.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_shows<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListShowsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListShowsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns one Show of a Shows Library, with its Seasons. Fails like `ListShows`, or with
    /// `not_found` if nothing in the Library is Matched to an Episode of that Show.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_show<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::GetShowRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::GetShowResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists one Season's Episodes, by episode number, each with its Versions and the caller's Watch
    /// State. Fails like `GetShow`, or with `not_found` if the Show has no such Season in the
    /// Library.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_episodes<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListEpisodesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListEpisodesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Lists the caller's Continue Watching: the Movies and Episodes whose position is past 0 and
    /// that they haven't played, most recent first by when their Watch State last changed. Only
    /// those Matched in some Library are listed, each in the oldest Library that has it. The Watch
    /// State of the rest is kept, so they are listed again once a rescan Matches them.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_continue_watching<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListContinueWatchingRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListContinueWatchingResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Searches every Library for the Movies, Shows and Episodes whose titles have each word of the
    /// query, ignoring case, accents and the marks that join words, so "amelie" finds Amélie and
    /// "spiderman" finds Spider-Man. Each word matches the start of a word, so a client can search
    /// as the User types. A last word of four digits after others is a year, which keeps only the
    /// titles released or aired that year, or with it in their title: "dune 2021" finds the 2021
    /// Dune. A Show is found by its own title, and an Episode by its own, not its Show's.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn search_libraries<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::SearchLibrariesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::SearchLibrariesResponse,
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
pub trait BrowseServiceExt: BrowseService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: BrowseService> BrowseServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                BROWSE_SERVICE_SERVICE_NAME,
                "ListMovies",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListMoviesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListMoviesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_movies(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListMoviesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(BROWSE_SERVICE_LIST_MOVIES_SPEC)
            .route_view_idempotent(
                BROWSE_SERVICE_SERVICE_NAME,
                "GetMovie",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetMovieRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetMovieRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_movie(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetMovieResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(BROWSE_SERVICE_GET_MOVIE_SPEC)
            .route_view_idempotent(
                BROWSE_SERVICE_SERVICE_NAME,
                "ListShows",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListShowsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListShowsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_shows(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListShowsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(BROWSE_SERVICE_LIST_SHOWS_SPEC)
            .route_view_idempotent(
                BROWSE_SERVICE_SERVICE_NAME,
                "GetShow",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::GetShowRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::GetShowRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_show(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::GetShowResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(BROWSE_SERVICE_GET_SHOW_SPEC)
            .route_view_idempotent(
                BROWSE_SERVICE_SERVICE_NAME,
                "ListEpisodes",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListEpisodesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListEpisodesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_episodes(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListEpisodesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(BROWSE_SERVICE_LIST_EPISODES_SPEC)
            .route_view_idempotent(
                BROWSE_SERVICE_SERVICE_NAME,
                "ListContinueWatching",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListContinueWatchingRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListContinueWatchingRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_continue_watching(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListContinueWatchingResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(BROWSE_SERVICE_LIST_CONTINUE_WATCHING_SPEC)
            .route_view_idempotent(
                BROWSE_SERVICE_SERVICE_NAME,
                "SearchLibraries",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::SearchLibrariesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::SearchLibrariesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.search_libraries(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::SearchLibrariesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(BROWSE_SERVICE_SEARCH_LIBRARIES_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct BrowseServiceRegisterMarker;
impl<S: BrowseService> ::connectrpc::ServiceRegister<BrowseServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as BrowseServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `BrowseService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = BrowseServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct BrowseServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: BrowseService> BrowseServiceServer<T> {
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
impl<T> Clone for BrowseServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: BrowseService> ::connectrpc::Dispatcher for BrowseServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.BrowseService/")?;
        match method {
            "ListMovies" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(BROWSE_SERVICE_LIST_MOVIES_SPEC),
                )
            }
            "GetMovie" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(BROWSE_SERVICE_GET_MOVIE_SPEC),
                )
            }
            "ListShows" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(BROWSE_SERVICE_LIST_SHOWS_SPEC),
                )
            }
            "GetShow" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(BROWSE_SERVICE_GET_SHOW_SPEC),
                )
            }
            "ListEpisodes" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(BROWSE_SERVICE_LIST_EPISODES_SPEC),
                )
            }
            "ListContinueWatching" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(BROWSE_SERVICE_LIST_CONTINUE_WATCHING_SPEC),
                )
            }
            "SearchLibraries" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(BROWSE_SERVICE_SEARCH_LIBRARIES_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.BrowseService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "ListMovies" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListMoviesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListMoviesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListMoviesRequest,
                    >::from_parts(&req, &body);
                    svc.list_movies(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ListMoviesResponse>(format)
                })
            }
            "GetMovie" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetMovieRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetMovieRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetMovieRequest,
                    >::from_parts(&req, &body);
                    svc.get_movie(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::GetMovieResponse>(format)
                })
            }
            "ListShows" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListShowsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListShowsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListShowsRequest,
                    >::from_parts(&req, &body);
                    svc.list_shows(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ListShowsResponse>(format)
                })
            }
            "GetShow" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::GetShowRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::GetShowRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::GetShowRequest,
                    >::from_parts(&req, &body);
                    svc.get_show(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::GetShowResponse>(format)
                })
            }
            "ListEpisodes" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListEpisodesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListEpisodesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListEpisodesRequest,
                    >::from_parts(&req, &body);
                    svc.list_episodes(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ListEpisodesResponse,
                        >(format)
                })
            }
            "ListContinueWatching" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListContinueWatchingRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListContinueWatchingRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListContinueWatchingRequest,
                    >::from_parts(&req, &body);
                    svc.list_continue_watching(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ListContinueWatchingResponse,
                        >(format)
                })
            }
            "SearchLibraries" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::SearchLibrariesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::SearchLibrariesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::SearchLibrariesRequest,
                    >::from_parts(&req, &body);
                    svc.search_libraries(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::SearchLibrariesResponse,
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
        let Some(method) = path.strip_prefix("nama.v1.BrowseService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.BrowseService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.BrowseService/") else {
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
/// let client = BrowseServiceClient::new(conn, config);
/// let response = client.list_movies(request).await?;
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
/// let client = BrowseServiceClient::new(http, config);
/// let response = client.list_movies(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.list_movies(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.list_movies(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct BrowseServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> BrowseServiceClient<T>
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
    /// Call the ListMovies RPC. Sends a request to /nama.v1.BrowseService/ListMovies.
    pub async fn list_movies(
        &self,
        request: crate::messages::nama::v1::ListMoviesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListMoviesResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_movies_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListMovies RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_movies_with_options(
        &self,
        request: crate::messages::nama::v1::ListMoviesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListMoviesResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                BROWSE_SERVICE_LIST_MOVIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetMovie RPC. Sends a request to /nama.v1.BrowseService/GetMovie.
    pub async fn get_movie(
        &self,
        request: crate::messages::nama::v1::GetMovieRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetMovieResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_movie_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetMovie RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_movie_with_options(
        &self,
        request: crate::messages::nama::v1::GetMovieRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetMovieResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                BROWSE_SERVICE_GET_MOVIE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListShows RPC. Sends a request to /nama.v1.BrowseService/ListShows.
    pub async fn list_shows(
        &self,
        request: crate::messages::nama::v1::ListShowsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListShowsResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_shows_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListShows RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_shows_with_options(
        &self,
        request: crate::messages::nama::v1::ListShowsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListShowsResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                BROWSE_SERVICE_LIST_SHOWS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetShow RPC. Sends a request to /nama.v1.BrowseService/GetShow.
    pub async fn get_show(
        &self,
        request: crate::messages::nama::v1::GetShowRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetShowResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_show_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the GetShow RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_show_with_options(
        &self,
        request: crate::messages::nama::v1::GetShowRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::GetShowResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                BROWSE_SERVICE_GET_SHOW_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListEpisodes RPC. Sends a request to /nama.v1.BrowseService/ListEpisodes.
    pub async fn list_episodes(
        &self,
        request: crate::messages::nama::v1::ListEpisodesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListEpisodesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_episodes_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListEpisodes RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_episodes_with_options(
        &self,
        request: crate::messages::nama::v1::ListEpisodesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListEpisodesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                BROWSE_SERVICE_LIST_EPISODES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListContinueWatching RPC. Sends a request to /nama.v1.BrowseService/ListContinueWatching.
    pub async fn list_continue_watching(
        &self,
        request: crate::messages::nama::v1::ListContinueWatchingRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListContinueWatchingResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_continue_watching_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListContinueWatching RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_continue_watching_with_options(
        &self,
        request: crate::messages::nama::v1::ListContinueWatchingRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListContinueWatchingResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                BROWSE_SERVICE_LIST_CONTINUE_WATCHING_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SearchLibraries RPC. Sends a request to /nama.v1.BrowseService/SearchLibraries.
    pub async fn search_libraries(
        &self,
        request: crate::messages::nama::v1::SearchLibrariesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SearchLibrariesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.search_libraries_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SearchLibraries RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn search_libraries_with_options(
        &self,
        request: crate::messages::nama::v1::SearchLibrariesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::SearchLibrariesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                BROWSE_SERVICE_SEARCH_LIBRARIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
