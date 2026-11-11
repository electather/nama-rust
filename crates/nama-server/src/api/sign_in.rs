//! Signing Devices in, and saying who is signed in: `nama.v1.SignInService`.

use std::{future, time::Instant};

use axum::http::{
    HeaderMap, HeaderValue,
    header::{RETRY_AFTER, SET_COOKIE, USER_AGENT},
};
use buffa::EnumValue;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use nama_proto::nama::v1::{
    ApproveSignInCodeRequest, ApproveSignInCodeResponse, LookUpSignInCodeRequest,
    LookUpSignInCodeResponse, PollSignInCodeRequest, PollSignInCodeResponse, ReauthenticateRequest,
    ReauthenticateResponse, RefusalReason, SessionDelivery, SetupRequest, SetupResponse,
    SignInCodeStatus, SignInRequest, SignInResponse, SignInService, SignOutRequest,
    SignOutResponse, StartSignInCodeRequest, StartSignInCodeResponse, WhoAmIRequest,
    WhoAmIResponse, reauthenticate_request::Proof,
};

use crate::{
    rate_limits::{Blocked, SignInCodeLimits},
    sessions::session_cookie,
    sign_in::{self, SignIn},
    sign_in_codes::{self, Poll, SignInCodes},
    users::{self, Caller, NewDevice, SignedIn, Users},
};

use super::{
    caller, client_ip, database_failed, refuse, signed_out, unknown_enum_value,
    users::{device_to_proto, user_to_proto},
};

/// `nama.v1.SignInService`: signing Devices in, and saying who is signed in.
pub(super) struct SignInServiceHandler {
    pub(super) users: Users,
    pub(super) sign_in: SignIn,
    pub(super) sign_in_codes: SignInCodes,
    /// The recent failed Sign-in Code lookups and approvals, which slow guessing codes down.
    pub(super) sign_in_code_limits: SignInCodeLimits,
}

impl SignInServiceHandler {
    /// Looks up or approves a Sign-in Code with `use_code`, as the caller of `ctx` at the time it
    /// is given, unless their Device has failed too often lately. Only a code that is unknown or
    /// expired counts as a failure: that is what a guess gets.
    fn with_sign_in_code_limit<T>(
        &self,
        ctx: &RequestContext,
        use_code: impl FnOnce(&Caller, Instant) -> Result<T, sign_in_codes::Error>,
    ) -> Result<T, ConnectError> {
        let caller = caller(ctx)?;
        let now = Instant::now();
        let attempt = self.sign_in_code_limits.admit(caller.device.id, now)?;
        let used = use_code(caller, now);
        if !matches!(used, Err(sign_in_codes::Error::NotFound)) {
            self.sign_in_code_limits.wasnt_a_guess(&attempt);
        }
        Ok(used?)
    }
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
        let wants_token = wants_token(request.delivery)?;
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
        Ok(hand_out_session(
            signed_in,
            wants_token,
            &ctx,
            |caller, token| SetupResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            },
        ))
    }

    async fn sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SignInRequest>,
    ) -> ServiceResult<SignInResponse> {
        let request = request.to_owned_message();
        let wants_token = wants_token(request.delivery)?;
        let proof = sign_in::Proof::Password {
            name: request.name,
            password: request.password,
        };
        let device = new_device(request.device_name, &ctx)?;
        let signed_in = self.sign_in.sign_in(proof, device, Instant::now()).await?;
        Ok(hand_out_session(
            signed_in,
            wants_token,
            &ctx,
            |caller, token| SignInResponse {
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            },
        ))
    }

    async fn sign_out(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, SignOutRequest>,
    ) -> ServiceResult<SignOutResponse> {
        let device = caller(&ctx)?.device.id;
        self.users.sign_out(device).await.map_err(database_failed)?;
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
        let attempt = self
            .sign_in
            .limits
            .admit(client_ip(&ctx)?, Some(&caller.user.name))?;
        let Some(Proof::Password(password)) = request.to_owned_message().proof else {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
                "the proof of who you are is missing",
            )
            .field("proof")
            .into());
        };
        let reauthenticated = self.users.reauthenticate(caller, &password).await;
        // The Device is only found signed out after the password checked out, so that is no guess.
        if matches!(reauthenticated, Ok(()) | Err(users::Error::SignedOut)) {
            self.sign_in.limits.succeeded(&attempt);
        }
        reauthenticated?;
        Response::ok(ReauthenticateResponse::default())
    }

    fn start_sign_in_code<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, StartSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<StartSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let request = request.to_owned_message();
        let started = wants_token(request.delivery).and_then(|wants_token| {
            let device_name =
                users::checked_name("device_name", "a Device's name", &request.device_name)?;
            Ok(self
                .sign_in_codes
                .start(device_name, wants_token, Instant::now())?)
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
        let looked_up = self
            .with_sign_in_code_limit(&ctx, |_, now| self.sign_in_codes.look_up(request.code, now));
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
        let approved = self.with_sign_in_code_limit(&ctx, |caller, now| {
            self.sign_in_codes.approve(request.code, &caller.user, now)
        });
        future::ready(approved.and_then(|()| Response::ok(ApproveSignInCodeResponse::default())))
    }

    async fn poll_sign_in_code(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, PollSignInCodeRequest>,
    ) -> ServiceResult<PollSignInCodeResponse> {
        // Before `poll`, which uses up an approved code: failing after it would lose the sign-in.
        let ip_address = client_ip(&ctx)?;
        let poll = self
            .sign_in_codes
            .poll(request.handle, Instant::now())
            .ok_or_else(|| {
                refuse(
                    RefusalReason::REFUSAL_REASON_HANDLE_NOT_FOUND,
                    "no sign-in is waiting on that handle",
                )
                .field("handle")
            })?;
        let (user, device_name, wants_token) = match poll {
            Poll::Waiting => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_WAITING),
            Poll::Expired => return poll_status(SignInCodeStatus::SIGN_IN_CODE_STATUS_EXPIRED),
            Poll::Approved {
                user,
                device_name,
                wants_token,
            } => (user, device_name, wants_token),
        };
        let device = NewDevice {
            name: device_name,
            ip_address,
            user_agent: user_agent(ctx.headers()),
        };
        let signed_in = self.users.sign_in_as(user, None, device).await?;
        Ok(hand_out_session(
            signed_in,
            wants_token,
            &ctx,
            |caller, token| PollSignInCodeResponse {
                status: SignInCodeStatus::SIGN_IN_CODE_STATUS_SIGNED_IN.into(),
                user: user_to_proto(caller.user).into(),
                device: device_to_proto(caller.device, true).into(),
                token,
                ..Default::default()
            },
        ))
    }
}

/// A `PollSignInCode` answer that carries only `status`.
fn poll_status(status: SignInCodeStatus) -> ServiceResult<PollSignInCodeResponse> {
    Response::ok(PollSignInCodeResponse {
        status: status.into(),
        ..Default::default()
    })
}

/// Whether a client asking for `delivery` wants its session token in the response body, rather
/// than as a cookie.
fn wants_token(delivery: EnumValue<SessionDelivery>) -> Result<bool, ConnectError> {
    match delivery {
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_TOKEN) => Ok(true),
        EnumValue::Known(SessionDelivery::SESSION_DELIVERY_COOKIE) => Ok(false),
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

/// The response `body` makes for a Device just `signed_in`, with its session token in the body for
/// a client that `wants_token`, or else in a cookie and not in the body.
fn hand_out_session<M>(
    signed_in: SignedIn,
    wants_token: bool,
    ctx: &RequestContext,
    body: impl FnOnce(Caller, String) -> M,
) -> Response<M> {
    let SignedIn { caller, token } = signed_in;
    if wants_token {
        Response::new(body(caller, token))
    } else {
        let cookie = session_cookie(&token, ctx.extensions());
        Response::new(body(caller, String::new())).with_header(SET_COOKIE, cookie)
    }
}

/// The Connect error a client gets for a rate-limited attempt, with the delay to wait in
/// `Retry-After`.
impl From<Blocked> for ConnectError {
    fn from(blocked: Blocked) -> Self {
        let retry_after_secs = blocked.retry_after_secs();
        let headers = HeaderMap::from_iter([(RETRY_AFTER, HeaderValue::from(retry_after_secs))]);
        Self::from(
            refuse(
                RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
                blocked.to_string(),
            )
            .param("retryAfterSeconds", &retry_after_secs),
        )
        .with_headers(headers)
    }
}

/// The Connect error a client gets for a Sign-in Code that couldn't be started, looked up or
/// approved. A random number generator that failed is logged, not sent.
impl From<sign_in_codes::Error> for ConnectError {
    fn from(err: sign_in_codes::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            sign_in_codes::Error::NotFound => {
                refuse(R::REFUSAL_REASON_SIGN_IN_CODE_NOT_FOUND, message).field("code")
            }
            sign_in_codes::Error::AlreadyApproved => {
                refuse(R::REFUSAL_REASON_SIGN_IN_CODE_ALREADY_APPROVED, message).field("code")
            }
            sign_in_codes::Error::TooManyPending => {
                refuse(R::REFUSAL_REASON_TOO_MANY_PENDING_SIGN_INS, message)
            }
            sign_in_codes::Error::Random(_) => {
                tracing::error!("starting a Sign-in Code failed: {err}");
                return Self::internal("the Server failed");
            }
        };
        refused.into()
    }
}

/// The Connect error a client gets for a sign-in attempt that was refused or failed.
impl From<sign_in::Error> for ConnectError {
    fn from(err: sign_in::Error) -> Self {
        match err {
            sign_in::Error::Blocked(blocked) => blocked.into(),
            sign_in::Error::WrongSetupCode => refuse(
                RefusalReason::REFUSAL_REASON_WRONG_SETUP_CODE,
                err.to_string(),
            )
            .field("setup_code")
            .into(),
            sign_in::Error::Users(err) => err.into(),
        }
    }
}
