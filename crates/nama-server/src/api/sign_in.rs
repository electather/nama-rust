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
    rate_limits::Blocked,
    sessions::session_cookie,
    sign_in::{self, Polled, SignIn},
    sign_in_codes::{self, Delivery},
    users::{self, Caller, NewDevice, SetupCode, SignedIn, Users},
};

use super::{
    caller, client_ip, database_failed, refuse, signed_out, unknown_enum_value,
    users::{device_to_proto, user_to_proto},
};

/// `nama.v1.SignInService`: signing Devices in, and saying who is signed in.
pub(super) struct SignInServiceHandler {
    pub(super) users: Users,
    /// What `Setup` must be given. `None` once the Server started with Users.
    pub(super) setup_code: Option<SetupCode>,
    pub(super) sign_in: SignIn,
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
        let attempt = self.sign_in.limits.admit(client_ip(&ctx)?, None)?;
        let delivery = delivery(request.delivery)?;
        // The Server only made no code because it started with Users. Users are checked before the
        // code, so once there are any every attempt is refused as already set up, as documented.
        let setup_code = self.setup_code.as_ref().ok_or(users::Error::AlreadySetUp)?;
        if self.users.any().await.map_err(database_failed)? {
            return Err(users::Error::AlreadySetUp.into());
        }
        if !setup_code.matches(&request.setup_code) {
            return Err(refuse(
                RefusalReason::REFUSAL_REASON_WRONG_SETUP_CODE,
                "the Setup Code is wrong",
            )
            .field("setup_code")
            .into());
        }
        let device = sign_in::named(new_device(request.device_name, &ctx)?)?;
        let signed_in = self
            .users
            .create_first_admin(&request.name, &request.password, device)
            .await?;
        self.sign_in.limits.succeeded(&attempt);
        Ok(hand_out_session(
            signed_in,
            delivery,
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
        let delivery = delivery(request.delivery)?;
        let proof = sign_in::Proof::Password {
            name: request.name,
            password: request.password,
        };
        let device = new_device(request.device_name, &ctx)?;
        let signed_in = self.sign_in.sign_in(proof, device, Instant::now()).await?;
        Ok(hand_out_session(
            signed_in,
            delivery,
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
        ctx: RequestContext,
        request: ServiceRequest<'_, StartSignInCodeRequest>,
    ) -> impl Future<
        Output = ServiceResult<impl Encodable<StartSignInCodeResponse> + Send + use<'a>>,
    > + Send {
        let request = request.to_owned_message();
        let started = delivery(request.delivery).and_then(|delivery| {
            let user_agent = user_agent(ctx.headers());
            Ok(self.sign_in.start_code(
                request.device_name,
                user_agent.as_deref(),
                delivery,
                Instant::now(),
            )?)
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
        Ok(hand_out_session(
            signed_in,
            delivery,
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

/// The response `body` makes for a Device just `signed_in`, with its session token handed out by
/// `delivery`: in the body, or in a cookie and not in the body.
fn hand_out_session<M>(
    signed_in: SignedIn,
    delivery: Delivery,
    ctx: &RequestContext,
    body: impl FnOnce(Caller, String) -> M,
) -> Response<M> {
    let SignedIn { caller, token } = signed_in;
    match delivery {
        Delivery::Token => Response::new(body(caller, token)),
        Delivery::Cookie => {
            let cookie = session_cookie(&token, ctx.extensions());
            Response::new(body(caller, String::new())).with_header(SET_COOKIE, cookie)
        }
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

/// The Connect error a client gets for a Sign-in Code that couldn't be started, looked up,
/// approved or polled. A random number generator that failed is logged, not sent.
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
            sign_in_codes::Error::HandleNotFound => {
                refuse(R::REFUSAL_REASON_HANDLE_NOT_FOUND, message).field("handle")
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
            sign_in::Error::Users(err) => err.into(),
            sign_in::Error::Code(err) => err.into(),
        }
    }
}
