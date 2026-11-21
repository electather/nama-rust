///Shorthand for `OwnedView<ListJobsRequestView<'static>>`.
pub type OwnedListJobsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListJobsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListJobsResponseView<'static>>`.
pub type OwnedListJobsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListJobsResponseView<'static>,
>;
///Shorthand for `OwnedView<ListJobRunsRequestView<'static>>`.
pub type OwnedListJobRunsRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListJobRunsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListJobRunsResponseView<'static>>`.
pub type OwnedListJobRunsResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListJobRunsResponseView<'static>,
>;
///Shorthand for `OwnedView<ConfigureJobRequestView<'static>>`.
pub type OwnedConfigureJobRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ConfigureJobRequestView<'static>,
>;
///Shorthand for `OwnedView<ConfigureJobResponseView<'static>>`.
pub type OwnedConfigureJobResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ConfigureJobResponseView<'static>,
>;
///Shorthand for `OwnedView<PreviewJobScheduleRequestView<'static>>`.
pub type OwnedPreviewJobScheduleRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PreviewJobScheduleRequestView<'static>,
>;
///Shorthand for `OwnedView<PreviewJobScheduleResponseView<'static>>`.
pub type OwnedPreviewJobScheduleResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PreviewJobScheduleResponseView<'static>,
>;
///Shorthand for `OwnedView<RunJobRequestView<'static>>`.
pub type OwnedRunJobRequestView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RunJobRequestView<'static>,
>;
///Shorthand for `OwnedView<RunJobResponseView<'static>>`.
pub type OwnedRunJobResponseView = ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RunJobResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListJobsResponse>
for crate::messages::nama::v1::__buffa::view::ListJobsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListJobsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListJobsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListJobRunsResponse>
for crate::messages::nama::v1::__buffa::view::ListJobRunsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ListJobRunsResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ListJobRunsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::ConfigureJobResponse>
for crate::messages::nama::v1::__buffa::view::ConfigureJobResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::ConfigureJobResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::ConfigureJobResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::PreviewJobScheduleResponse>
for crate::messages::nama::v1::__buffa::view::PreviewJobScheduleResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::PreviewJobScheduleResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::PreviewJobScheduleResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::messages::nama::v1::RunJobResponse>
for crate::messages::nama::v1::__buffa::view::RunJobResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::messages::nama::v1::RunJobResponse>
for ::buffa::view::OwnedView<
    crate::messages::nama::v1::__buffa::view::RunJobResponseView<'static>,
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
pub const JOB_SERVICE_SERVICE_NAME: &str = "nama.v1.JobService";
/// Static [`Spec`](::connectrpc::Spec) for the `ListJobs` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const JOB_SERVICE_LIST_JOBS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.JobService/ListJobs",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ListJobRuns` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const JOB_SERVICE_LIST_JOB_RUNS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.JobService/ListJobRuns",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ConfigureJob` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const JOB_SERVICE_CONFIGURE_JOB_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.JobService/ConfigureJob",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Idempotent);
/// Static [`Spec`](::connectrpc::Spec) for the `PreviewJobSchedule` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const JOB_SERVICE_PREVIEW_JOB_SCHEDULE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.JobService/PreviewJobSchedule",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `RunJob` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const JOB_SERVICE_RUN_JOB_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/nama.v1.JobService/RunJob",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// The Jobs the enabled Plugins declare, which the Server runs on their schedules, and their Job
/// Runs (ADR 0079). Needs `PERMISSION_ADMIN`, and fails with `permission_denied` without it.
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
pub trait JobService: Send + Sync + 'static {
    /// Lists the Jobs of every enabled Plugin, by Plugin id and then in the order its Manifest
    /// declares them. A disabled Plugin's Jobs aren't listed; their Runs are kept for when it's
    /// enabled again.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_jobs<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListJobsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListJobsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns a Job's Runs, newest first: its last 50, whatever their outcome. A Job that never ran,
    /// or that no Plugin declares, has none.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_job_runs<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ListJobRunsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ListJobRunsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Turns a Job on or off and sets the schedule it runs on in place of its Manifest's, taking
    /// effect at once. A Job on and on its Manifest's schedule has nothing stored, so a Plugin
    /// update's new schedule reaches it: that is how a Job is reset to its default. Fails with
    /// `REFUSAL_REASON_JOB_NOT_FOUND` if no Plugin the Server has declares it, and
    /// `REFUSAL_REASON_SCHEDULE_INVALID` if `schedule` doesn't parse or is set for a Job without one
    /// of its own.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn configure_job<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::ConfigureJobRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::ConfigureJobResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Returns when `schedule` next fires, for showing before it is saved. Fails with
    /// `REFUSAL_REASON_SCHEDULE_INVALID` if it doesn't parse.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn preview_job_schedule<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::PreviewJobScheduleRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::PreviewJobScheduleResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Starts a Run of a Job now, scheduled or not, recorded as started by the caller, and returns
    /// once its Run is listed, without waiting for it to end. Fails with `failed_precondition` and
    /// `REFUSAL_REASON_JOB_RUNNING` while the Job has a Run going, and `not_found` and
    /// `REFUSAL_REASON_JOB_NOT_FOUND` if no enabled Plugin declares it.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn run_job<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::messages::nama::v1::RunJobRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::messages::nama::v1::RunJobResponse,
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
pub trait JobServiceExt: JobService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: JobService> JobServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                JOB_SERVICE_SERVICE_NAME,
                "ListJobs",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListJobsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListJobsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_jobs(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListJobsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(JOB_SERVICE_LIST_JOBS_SPEC)
            .route_view_idempotent(
                JOB_SERVICE_SERVICE_NAME,
                "ListJobRuns",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ListJobRunsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ListJobRunsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_job_runs(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ListJobRunsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(JOB_SERVICE_LIST_JOB_RUNS_SPEC)
            .route_view(
                JOB_SERVICE_SERVICE_NAME,
                "ConfigureJob",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::ConfigureJobRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::ConfigureJobRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.configure_job(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::ConfigureJobResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(JOB_SERVICE_CONFIGURE_JOB_SPEC)
            .route_view_idempotent(
                JOB_SERVICE_SERVICE_NAME,
                "PreviewJobSchedule",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::PreviewJobScheduleRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::PreviewJobScheduleRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.preview_job_schedule(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::messages::nama::v1::PreviewJobScheduleResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(JOB_SERVICE_PREVIEW_JOB_SCHEDULE_SPEC)
            .route_view(
                JOB_SERVICE_SERVICE_NAME,
                "RunJob",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::messages::nama::v1::__buffa::view::RunJobRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::messages::nama::v1::RunJobRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.run_job(ctx, sreq)
                                .await?
                                .encode::<crate::messages::nama::v1::RunJobResponse>(format)
                        }
                    })
                },
            )
            .with_spec(JOB_SERVICE_RUN_JOB_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct JobServiceRegisterMarker;
impl<S: JobService> ::connectrpc::ServiceRegister<JobServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as JobServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `JobService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = JobServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct JobServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: JobService> JobServiceServer<T> {
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
impl<T> Clone for JobServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: JobService> ::connectrpc::Dispatcher for JobServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("nama.v1.JobService/")?;
        match method {
            "ListJobs" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(JOB_SERVICE_LIST_JOBS_SPEC),
                )
            }
            "ListJobRuns" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(JOB_SERVICE_LIST_JOB_RUNS_SPEC),
                )
            }
            "ConfigureJob" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(JOB_SERVICE_CONFIGURE_JOB_SPEC),
                )
            }
            "PreviewJobSchedule" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(JOB_SERVICE_PREVIEW_JOB_SCHEDULE_SPEC),
                )
            }
            "RunJob" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(JOB_SERVICE_RUN_JOB_SPEC),
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
        let Some(method) = path.strip_prefix("nama.v1.JobService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "ListJobs" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListJobsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListJobsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListJobsRequest,
                    >::from_parts(&req, &body);
                    svc.list_jobs(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ListJobsResponse>(format)
                })
            }
            "ListJobRuns" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ListJobRunsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ListJobRunsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ListJobRunsRequest,
                    >::from_parts(&req, &body);
                    svc.list_job_runs(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::ListJobRunsResponse>(format)
                })
            }
            "ConfigureJob" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::ConfigureJobRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::ConfigureJobRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::ConfigureJobRequest,
                    >::from_parts(&req, &body);
                    svc.configure_job(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::ConfigureJobResponse,
                        >(format)
                })
            }
            "PreviewJobSchedule" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::PreviewJobScheduleRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::PreviewJobScheduleRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::PreviewJobScheduleRequest,
                    >::from_parts(&req, &body);
                    svc.preview_job_schedule(ctx, req)
                        .await?
                        .encode::<
                            crate::messages::nama::v1::PreviewJobScheduleResponse,
                        >(format)
                })
            }
            "RunJob" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::messages::nama::v1::RunJobRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::messages::nama::v1::__buffa::view::RunJobRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::messages::nama::v1::RunJobRequest,
                    >::from_parts(&req, &body);
                    svc.run_job(ctx, req)
                        .await?
                        .encode::<crate::messages::nama::v1::RunJobResponse>(format)
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
        let Some(method) = path.strip_prefix("nama.v1.JobService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.JobService/") else {
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
        let Some(method) = path.strip_prefix("nama.v1.JobService/") else {
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
/// let client = JobServiceClient::new(conn, config);
/// let response = client.list_jobs(request).await?;
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
/// let client = JobServiceClient::new(http, config);
/// let response = client.list_jobs(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.list_jobs(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.list_jobs(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct JobServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> JobServiceClient<T>
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
    /// Call the ListJobs RPC. Sends a request to /nama.v1.JobService/ListJobs.
    pub async fn list_jobs(
        &self,
        request: crate::messages::nama::v1::ListJobsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListJobsResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_jobs_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListJobs RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_jobs_with_options(
        &self,
        request: crate::messages::nama::v1::ListJobsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListJobsResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                JOB_SERVICE_LIST_JOBS_SPEC.with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListJobRuns RPC. Sends a request to /nama.v1.JobService/ListJobRuns.
    pub async fn list_job_runs(
        &self,
        request: crate::messages::nama::v1::ListJobRunsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListJobRunsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_job_runs_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListJobRuns RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_job_runs_with_options(
        &self,
        request: crate::messages::nama::v1::ListJobRunsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ListJobRunsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                JOB_SERVICE_LIST_JOB_RUNS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ConfigureJob RPC. Sends a request to /nama.v1.JobService/ConfigureJob.
    pub async fn configure_job(
        &self,
        request: crate::messages::nama::v1::ConfigureJobRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ConfigureJobResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.configure_job_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ConfigureJob RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn configure_job_with_options(
        &self,
        request: crate::messages::nama::v1::ConfigureJobRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::ConfigureJobResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                JOB_SERVICE_CONFIGURE_JOB_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the PreviewJobSchedule RPC. Sends a request to /nama.v1.JobService/PreviewJobSchedule.
    pub async fn preview_job_schedule(
        &self,
        request: crate::messages::nama::v1::PreviewJobScheduleRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::PreviewJobScheduleResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.preview_job_schedule_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the PreviewJobSchedule RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn preview_job_schedule_with_options(
        &self,
        request: crate::messages::nama::v1::PreviewJobScheduleRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::PreviewJobScheduleResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                JOB_SERVICE_PREVIEW_JOB_SCHEDULE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the RunJob RPC. Sends a request to /nama.v1.JobService/RunJob.
    pub async fn run_job(
        &self,
        request: crate::messages::nama::v1::RunJobRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::RunJobResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.run_job_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the RunJob RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn run_job_with_options(
        &self,
        request: crate::messages::nama::v1::RunJobRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::messages::nama::v1::__buffa::view::RunJobResponseView<'static>,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                JOB_SERVICE_RUN_JOB_SPEC.with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
