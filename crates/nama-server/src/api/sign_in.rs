//! Signing Devices in, and saying who is signed in: `nama.v1.SignInService`.

use std::{
    future,
    net::{IpAddr, SocketAddr},
    time::Instant,
};

use axum::http::{
    HeaderMap, HeaderValue,
    header::{RETRY_AFTER, SET_COOKIE, USER_AGENT},
};
use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use nama_proto::nama::v1::{
    AcceptInviteRequest, AcceptInviteResponse, ApproveSignInCodeRequest, ApproveSignInCodeResponse,
    LookUpSignInCodeRequest, LookUpSignInCodeResponse, PollSignInCodeRequest,
    PollSignInCodeResponse, ReauthenticateRequest, ReauthenticateResponse, RefusalReason,
    SessionDelivery, SetupRequest, SetupResponse, SignInCodeStatus, SignInRequest, SignInResponse,
    SignInService, SignInWithAccountRequest, SignInWithAccountResponse, SignOutRequest,
    SignOutResponse, StartAccountSignInRequest, StartAccountSignInResponse, StartSignInCodeRequest,
    StartSignInCodeResponse, WhoAmIRequest, WhoAmIResponse, reauthenticate_request::Proof,
};

use crate::{
    plugins::ExtensionPoints,
    server_settings::ServerUrl,
    sync::Syncing,
    users::{
        self, Blocked, Caller, Delivery, NewDevice, Polled, SignIn, SignInCodeError, SignInError,
        SignedIn, session_cookie,
    },
};
use nama_forwarded::RequestSource;

use super::{
    caller, database_failed, refuse, server_link, signed_out, unknown_enum_value,
    users::{device_to_proto, user_to_proto},
};

/// `nama.v1.SignInService`: signing Devices in, and saying who is signed in.
pub(super) struct SignInServiceHandler {
    pub(super) sign_in: SignIn,
    /// The Plugin that links the Server, through which a User signs in with their Account.
    pub(super) points: ExtensionPoints,
    /// Told of each sign-in.
    pub(super) syncing: Syncing,
    /// The Server URL, one of the Server Addresses a sign-in with an Account may return to.
    pub(super) server_url: ServerUrl,
    /// Where the Server listens, whose LAN address is the other Server Address a sign-in with an
    /// Account may return to.
    pub(super) listening: SocketAddr,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl SignInService for SignInServiceHandler {
    async fn setup(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetupRequest>,
    ) -> ServiceResult<SetupResponse> {
        let request = request.to_owned_message();
        let delivery = delivery(request.delivery)?;
        let device = new_device(request.device_name, &ctx)?;
        let signed_in = self
            .sign_in
            .setup(
                &request.setup_code,
                &request.name,
                &request.password,
                device,
                Instant::now(),
            )
            .await?;
        Ok(
            self.hand_out_session(signed_in, delivery, &ctx, |caller, token| SetupResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            }),
        )
    }

    async fn sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SignInRequest>,
    ) -> ServiceResult<SignInResponse> {
        let request = request.to_owned_message();
        let delivery = delivery(request.delivery)?;
        let proof = users::Proof::Password {
            name: request.name,
            password: request.password,
        };
        let device = new_device(request.device_name, &ctx)?;
        let signed_in = self.sign_in.sign_in(proof, device, Instant::now()).await?;
        Ok(
            self.hand_out_session(signed_in, delivery, &ctx, |caller, token| SignInResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            }),
        )
    }

    async fn sign_out(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, SignOutRequest>,
    ) -> ServiceResult<SignOutResponse> {
        let device = caller(&ctx)?.device.id;
        self.sign_in
            .sign_out(device)
            .await
            .map_err(database_failed)?;
        Ok(signed_out(SignOutResponse::default(), &ctx))
    }

    fn who_am_i<'a>(
        &'a self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, WhoAmIRequest>,
    ) -> impl Future<Output = ServiceResult<impl Encodable<WhoAmIResponse> + Send + use<'a>>> + Send
    {
        future::ready(caller(&ctx).and_then(|caller| {
            Response::ok(WhoAmIResponse {
                user: user_to_proto(caller.user.clone()).into(),
                device: device_to_proto(caller.device.clone(), true).into(),
                ..Default::default()
            })
        }))
    }

    async fn reauthenticate(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ReauthenticateRequest>,
    ) -> ServiceResult<ReauthenticateResponse> {
        let caller = caller(&ctx)?;
        let (ip, now) = (client_ip(&ctx)?, Instant::now());
        let request = request.to_owned_message();
        let proof = match request.proof {
            Some(Proof::AccountSignInHandle(handle)) => {
                let approved = server_link::account_signing_in(
                    &self.points,
                    &handle,
                    &request.account_sign_in_code,
                    "account_sign_in_handle",
                );
                self.sign_in
                    .reauthenticate_with_account(caller, approved, ip, now)
                    .await?;
                return Response::ok(ReauthenticateResponse::default());
            }
            Some(Proof::Password(password)) => Some(users::Proof::Password {
                name: caller.user.name.clone(),
                password,
            }),
            None => None,
        };
        self.sign_in.reauthenticate(caller, proof, ip, now).await?;
        Response::ok(ReauthenticateResponse::default())
    }

    fn start_sign_in_code<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, StartSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<StartSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let request = request.to_owned_message();
        let started = delivery(request.delivery).and_then(|delivery| {
            let device = new_device(request.device_name, &ctx)?;
            Ok(self.sign_in.start_code(device, delivery, Instant::now())?)
        });
        future::ready(started.and_then(|started| {
            Response::ok(StartSignInCodeResponse {
                code: started.code,
                handle: started.handle,
                ..Default::default()
            })
        }))
    }

    fn look_up_sign_in_code<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, LookUpSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<LookUpSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let looked_up = caller(&ctx).and_then(|caller| {
            Ok(self
                .sign_in
                .look_up_code(caller, request.code, Instant::now())?)
        });
        future::ready(looked_up.and_then(|device_name| {
            Response::ok(LookUpSignInCodeResponse {
                device_name,
                ..Default::default()
            })
        }))
    }

    fn approve_sign_in_code<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, ApproveSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<ApproveSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let approved = caller(&ctx).and_then(|caller| {
            Ok(self
                .sign_in
                .approve_code(caller, request.code, Instant::now())?)
        });
        future::ready(approved.and_then(|()| Response::ok(ApproveSignInCodeResponse::default())))
    }

    async fn poll_sign_in_code(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PollSignInCodeRequest>,
    ) -> ServiceResult<PollSignInCodeResponse> {
        // Before `poll_code`, which uses up an approved code: failing after it would lose the sign-in.
        let ip_address = client_ip(&ctx)?;
        let polled = self
            .sign_in
            .poll_code(
                request.handle,
                ip_address,
                user_agent(ctx.headers()),
                Instant::now(),
            )
            .await?;
        let (signed_in, delivery) = match polled {
            Polled::Waiting => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_WAITING),
            Polled::Expired => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_EXPIRED),
            Polled::SignedIn {
                signed_in,
                delivery,
            } => (signed_in, delivery),
        };
        Ok(
            self.hand_out_session(signed_in, delivery, &ctx, |caller, token| {
                PollSignInCodeResponse {
                    status: SignInCodeStatus::SIGN_IN_CODE_STATUS_SIGNED_IN.into(),
                    user: user_to_proto(caller.user).into(),
                    device: device_to_proto(caller.device, true).into(),
                    token,
                    ..Default::default()
                }
            }),
        )
    }

    async fn accept_invite(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, AcceptInviteRequest>,
    ) -> ServiceResult<AcceptInviteResponse> {
        let request = request.to_owned_message();
        let delivery = delivery(request.delivery)?;
        let device = new_device(request.device_name, &ctx)?;
        let signed_in = self
            .sign_in
            .accept_invite(
                &request.token,
                &request.name,
                &request.password,
                device,
                Instant::now(),
            )
            .await?;
        Ok(
            self.hand_out_session(signed_in, delivery, &ctx, |caller, token| {
                AcceptInviteResponse {
                    user: user_to_proto(caller.user).into(),
                    device: device_to_proto(caller.device, true).into(),
                    token,
                    ..Default::default()
                }
            }),
        )
    }

    async fn start_account_sign_in(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StartAccountSignInRequest>,
    ) -> ServiceResult<StartAccountSignInResponse> {
        server_link::start_account_sign_in(
            &self.points,
            request.return_url,
            &self.server_url,
            self.listening,
        )
        .await
    }

    async fn sign_in_with_account(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SignInWithAccountRequest>,
    ) -> ServiceResult<SignInWithAccountResponse> {
        let request = request.to_owned_message();
        let delivery = delivery(request.delivery)?;
        let device = new_device(request.device_name, &ctx)?;
        let user =
            server_link::account_signing_in(&self.points, &request.handle, &request.code, "handle")
                .await?;
        let signed_in = self.sign_in.sign_in_with_account(user, device).await?;
        Ok(
            self.hand_out_session(signed_in, delivery, &ctx, |caller, token| {
                SignInWithAccountResponse {
                    user: user_to_proto(caller.user).into(),
                    device: device_to_proto(caller.device, true).into(),
                    token,
                    ..Default::default()
                }
            }),
        )
    }
}

/// A `PollSignInCode` answer that carries only `status`.
fn poll_status(status: SignInCodeStatus) -> ServiceResult<PollSignInCodeResponse> {
    Response::ok(PollSignInCodeResponse {
        status: status.into(),
        ..Default::default()
    })
}

/// How a client asking for `delivery` wants its session handed out.
fn delivery(delivery: EnumValue<SessionDelivery>) -> Result<Delivery, ConnectError> {
    match delivery {
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_TOKEN) => Ok(Delivery::Token),
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_COOKIE) => Ok(Delivery::Cookie),
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_UNSPECIFIED) | EnumValue::Unknown(_) => {
            Err(unknown_enum_value(
                "delivery",
                "the session delivery must be cookie or token",
            ))
        }
    }
}

/// The Device that signs in with the request `ctx`, under the name it gave, which may be blank.
fn new_device(name: String, ctx: &RequestContext) -> Result<NewDevice, ConnectError> {
    Ok(NewDevice {
        name,
        ip_address: client_ip(ctx)?,
        user_agent: user_agent(ctx.headers()),
    })
}

/// The `User-Agent` `headers` carry, cut short, since any client can send a long one and it is
/// stored with the Device.
fn user_agent(headers: &HeaderMap) -> Option<String> {
    const LONGEST: usize = 512;
    let value = headers.get(USER_AGENT)?.to_str().ok()?;
    Some(value.chars().take(LONGEST).collect())
}

impl SignInServiceHandler {
    /// The response `body` makes for a Device just `signed_in`, with its session token handed out
    /// by `delivery`: in the body, or in a cookie and not in the body. Tells `syncing` too, as
    /// every sign-in does.
    fn hand_out_session<M>(
        &self,
        signed_in: SignedIn,
        delivery: Delivery,
        ctx: &RequestContext,
        body: impl FnOnce(Caller, String) -> M,
    ) -> Response<M> {
        let SignedIn { caller, token } = signed_in;
        self.syncing.signed_in(caller.user.id);
        match delivery {
            Delivery::Token => Response::new(body(caller, token)),
            Delivery::Cookie => {
                let cookie = session_cookie(&token, ctx.extensions());
                Response::new(body(caller, String::new())).with_header(SET_COOKIE, cookie)
            }
        }
    }
}

/// The Connect error a client gets for a rate-limited attempt, with the delay to wait in
/// `Retry-After`.
impl From<Blocked> for ConnectError {
    fn from(blocked: Blocked) -> Self {
        rate_limited(
            RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
            blocked.to_string(),
            &blocked,
        )
    }
}

/// The Connect error refusing a request for `reason` until `blocked` lets it through, with the
/// delay to wait in both `Retry-After` and the Refusal's `retryAfterSeconds`.
fn rate_limited(reason: RefusalReason, message: String, blocked: &Blocked) -> ConnectError {
    let retry_after_secs = blocked.retry_after_secs();
    let headers = HeaderMap::from_iter([(RETRY_AFTER, HeaderValue::from(retry_after_secs))]);
    ConnectError::from(refuse(reason, message).param("retryAfterSeconds", &retry_after_secs))
        .with_headers(headers)
}

/// The Connect error a client gets for a Sign-in Code that couldn't be started, looked up,
/// approved or polled. A random number generator that failed is logged, not sent.
impl From<SignInCodeError> for ConnectError {
    fn from(err: SignInCodeError) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            SignInCodeError::NotFound => {
                refuse(R::REFUSAL_REASON_SIGN_IN_CODE_NOT_FOUND, message).field("code")
            }
            SignInCodeError::AlreadyApproved => {
                refuse(R::REFUSAL_REASON_SIGN_IN_CODE_ALREADY_APPROVED, message).field("code")
            }
            SignInCodeError::HandleNotFound => {
                refuse(R::REFUSAL_REASON_HANDLE_NOT_FOUND, message).field("handle")
            }
            SignInCodeError::TooManyPending => {
                refuse(R::REFUSAL_REASON_TOO_MANY_PENDING_SIGN_INS, message)
            }
            SignInCodeError::Random(_) => {
                tracing::error!("starting a Sign-in Code failed: {err}");
                return Self::internal("the Server failed");
            }
        };
        refused.into()
    }
}

/// The Connect error a client gets for a sign-in attempt that was refused or failed.
impl From<SignInError> for ConnectError {
    fn from(err: SignInError) -> Self {
        match err {
            SignInError::Blocked(blocked) => blocked.into(),
            SignInError::TooManyCodeStarts(ref blocked) => rate_limited(
                RefusalReason::REFUSAL_REASON_TOO_MANY_PENDING_SIGN_INS,
                err.to_string(),
                blocked,
            ),
            SignInError::WrongSetupCode => refuse(
                RefusalReason::REFUSAL_REASON_WRONG_SETUP_CODE,
                err.to_string(),
            )
            .field("setup_code")
            .into(),
            SignInError::Users(err) => err.into(),
            SignInError::Code(err) => err.into(),
            err @ SignInError::ProofMissing => refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                err.to_string(),
            )
            .field("proof")
            .into(),
            err @ SignInError::WrongAccount => {
                refuse(RefusalReason::REFUSAL_REASON_WRONG_ACCOUNT, err.to_string())
                    .field("account_sign_in_handle")
                    .into()
            }
        }
    }
}

/// The client IP address `ctx` came from, which rate limits count by.
pub(super) fn client_ip(ctx: &RequestContext) -> Result<IpAddr, ConnectError> {
    let source = ctx.extensions().get::<RequestSource>();
    source.map(|source| source.client_ip).ok_or_else(|| {
        tracing::error!("a request came without its peer's address");
        ConnectError::internal("the Server failed")
    })
}
