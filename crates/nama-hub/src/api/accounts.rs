//! Creating an Account, signing it in, with its password or a Sign-in Link, and out, and setting
//! its password: `nama.hub.v1.AccountService`.

use std::{future, net::IpAddr, time::Instant};

use axum::http::{
    Extensions, HeaderMap, HeaderValue,
    header::{RETRY_AFTER, SET_COOKIE},
};
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use nama_forwarded::RequestSource;
use nama_hub_proto::nama::hub::v1::{
    self as proto, AccountService, CreateAccountRequest, CreateAccountResponse, GetAccountRequest,
    GetAccountResponse, GetSignInLinkRequest, GetSignInLinkResponse, RefusalReason,
    SendSignInLinkRequest, SendSignInLinkResponse, SetPasswordRequest, SetPasswordResponse,
    SignInRequest, SignInResponse, SignInWithLinkRequest, SignInWithLinkResponse, SignOutRequest,
    SignOutResponse,
};

use super::{hub_failed, refuse};
use crate::accounts::{
    self, Account, AccountSession, Accounts, Blocked, EMAIL_LENGTH, PASSWORD_LENGTH,
    RECENT_AUTHENTICATION_MINS, SESSION_DAYS, SignInLink, SignedIn, SignedInByLink,
};

/// The cookie that holds a browser's session token.
pub(super) const COOKIE_NAME: &str = "nama_hub_session";

/// `nama.hub.v1.AccountService`: creating an Account, signing it in and out, and setting its
/// password.
pub(super) struct AccountServiceHandler {
    pub(super) accounts: Accounts,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned response is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl AccountService for AccountServiceHandler {
    async fn create_account(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, CreateAccountRequest>,
    ) -> ServiceResult<CreateAccountResponse> {
        let signed_in = self
            .accounts
            .create(
                request.email,
                request.password,
                client_ip(&ctx)?,
                Instant::now(),
            )
            .await
            .map_err(refused)?;
        Ok(hand_out_session(signed_in, &ctx, |account| {
            CreateAccountResponse {
                account: account.into(),
                ..Default::default()
            }
        }))
    }

    async fn sign_in(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SignInRequest>,
    ) -> ServiceResult<SignInResponse> {
        let signed_in = self
            .accounts
            .sign_in(
                request.email,
                request.password,
                client_ip(&ctx)?,
                Instant::now(),
            )
            .await
            .map_err(refused)?;
        Ok(hand_out_session(signed_in, &ctx, |account| {
            SignInResponse {
                account: account.into(),
                ..Default::default()
            }
        }))
    }

    fn send_sign_in_link<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SendSignInLinkRequest>,
    ) -> impl Future<Output = ServiceResult<impl Encodable<SendSignInLinkResponse> + Send + use<'a>>>
    + Send {
        let queued = client_ip(&ctx).and_then(|ip| {
            self.accounts
                .send_sign_in_link(request.email, request.continue_to, ip, Instant::now())
                .map_err(refused)
        });
        future::ready(queued.and_then(|()| Response::ok(SendSignInLinkResponse::default())))
    }

    async fn get_sign_in_link(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, GetSignInLinkRequest>,
    ) -> ServiceResult<GetSignInLinkResponse> {
        let SignInLink {
            account,
            email_proven,
        } = self
            .accounts
            .sign_in_link(request.token)
            .await
            .map_err(refused)?;
        Response::ok(GetSignInLinkResponse {
            account: account_to_proto(account).into(),
            email_proven,
            ..Default::default()
        })
    }

    async fn sign_in_with_link(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SignInWithLinkRequest>,
    ) -> ServiceResult<SignInWithLinkResponse> {
        let SignedInByLink {
            signed_in,
            continue_to,
        } = self
            .accounts
            .sign_in_with_link(request.token, ctx.extensions().get::<AccountSession>())
            .await
            .map_err(refused)?;
        Ok(hand_out_session(signed_in, &ctx, |account| {
            SignInWithLinkResponse {
                account: account.into(),
                continue_to,
                ..Default::default()
            }
        }))
    }

    async fn set_password(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SetPasswordRequest>,
    ) -> ServiceResult<SetPasswordResponse> {
        self.accounts
            .set_password(signed_in(&ctx)?, request.password)
            .await
            .map_err(refused)?;
        Response::ok(SetPasswordResponse::default())
    }

    async fn sign_out(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, SignOutRequest>,
    ) -> ServiceResult<SignOutResponse> {
        self.accounts
            .sign_out(signed_in(&ctx)?)
            .await
            .map_err(|err| hub_failed(&err))?;
        let cleared = cookie("", 0, ctx.extensions());
        Ok(Response::new(SignOutResponse::default()).with_header(SET_COOKIE, cleared))
    }

    async fn get_account(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, GetAccountRequest>,
    ) -> ServiceResult<GetAccountResponse> {
        let account = &signed_in(&ctx)?.account;
        let has_password = self
            .accounts
            .has_password(account.id)
            .await
            .map_err(|err| hub_failed(&err))?;
        Response::ok(GetAccountResponse {
            account: account_to_proto(account.clone()).into(),
            has_password,
            ..Default::default()
        })
    }
}

/// The address of the client that sent the request `ctx` is for (ADR 0018).
fn client_ip(ctx: &RequestContext) -> Result<IpAddr, ConnectError> {
    Ok(ctx
        .extensions()
        .get::<RequestSource>()
        .ok_or_else(|| hub_failed(&"a request came without its peer's address"))?
        .client_ip)
}

/// The session the request `ctx` is for, or `REFUSAL_REASON_SIGN_IN_REQUIRED`.
pub(super) fn signed_in(ctx: &RequestContext) -> Result<&AccountSession, ConnectError> {
    ctx.extensions().get::<AccountSession>().ok_or_else(|| {
        refuse(
            RefusalReason::REFUSAL_REASON_SIGN_IN_REQUIRED,
            "sign in to an Account first",
        )
        .into()
    })
}

/// The response `body` makes for an Account just signed in, with its token in a cookie and never
/// in the body, so page scripts can't read it (ADR 0015).
fn hand_out_session<M>(
    SignedIn { account, token }: SignedIn,
    ctx: &RequestContext,
    body: impl FnOnce(proto::Account) -> M,
) -> Response<M> {
    let max_age = i64::from(SESSION_DAYS) * 24 * 60 * 60;
    Response::new(body(account_to_proto(account)))
        .with_header(SET_COOKIE, cookie(&token, max_age, ctx.extensions()))
}

/// A session cookie holding `value` for `max_age` seconds. `Secure` only when the request whose
/// `extensions` these are arrived over HTTPS, since a browser drops a `Secure` cookie sent over
/// plain HTTP, as the Hub is reached while developing it (ADR 0015, ADR 0018).
fn cookie(value: &str, max_age: i64, extensions: &Extensions) -> String {
    let secure = extensions
        .get::<RequestSource>()
        .is_some_and(|source| source.https);
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}{secure}")
}

fn account_to_proto(account: Account) -> proto::Account {
    proto::Account {
        id: account.id.to_string(),
        email: account.email,
        ..Default::default()
    }
}

/// The Connect error a client gets for an Account that couldn't be created, signed in or given a
/// password, or a Sign-in Link that couldn't be sent or used.
#[expect(
    clippy::needless_pass_by_value,
    reason = "by value, so callers can write `.map_err(refused)`"
)]
fn refused(err: accounts::Error) -> ConnectError {
    use RefusalReason as R;
    use accounts::Error as E;
    let message = err.to_string();
    let refused = match err {
        E::EmailTaken => refuse(R::REFUSAL_REASON_EMAIL_TAKEN, message).field("email"),
        E::EmailInvalid => refuse(R::REFUSAL_REASON_EMAIL_INVALID, message)
            .field("email")
            .param("maxLength", &EMAIL_LENGTH),
        E::PasswordLength => refuse(R::REFUSAL_REASON_PASSWORD_LENGTH, message)
            .field("password")
            .param("minLength", PASSWORD_LENGTH.start())
            .param("maxLength", PASSWORD_LENGTH.end()),
        E::RecentAuthenticationRequired => {
            refuse(R::REFUSAL_REASON_RECENT_AUTHENTICATION_REQUIRED, message)
                .param("recentMinutes", &RECENT_AUTHENTICATION_MINS)
        }
        E::WrongEmailOrPassword => refuse(R::REFUSAL_REASON_WRONG_EMAIL_OR_PASSWORD, message),
        E::SignInLinksOff => refuse(R::REFUSAL_REASON_SIGN_IN_LINKS_OFF, message),
        E::SignInLinkExpired => refuse(R::REFUSAL_REASON_SIGN_IN_LINK_EXPIRED, message),
        E::Blocked(ref blocked) => return too_many_attempts(blocked),
        // ADR 0019: no Refusal, but `unavailable`, since the queue drains and a retry may work.
        E::MailQueueFull => {
            tracing::warn!("{message}");
            return ConnectError::unavailable(message);
        }
        E::Password(_) | E::Random(_) | E::Database(_) => {
            return hub_failed(&err);
        }
    };
    refused.into()
}

/// Refuses a request until `blocked` lets it through, with the delay to wait in both `Retry-After`
/// and the Refusal's `retryAfterSeconds`.
fn too_many_attempts(blocked: &Blocked) -> ConnectError {
    let retry_after_secs = blocked.retry_after_secs();
    let headers = HeaderMap::from_iter([(RETRY_AFTER, HeaderValue::from(retry_after_secs))]);
    ConnectError::from(
        refuse(
            RefusalReason::REFUSAL_REASON_TOO_MANY_ATTEMPTS,
            blocked.to_string(),
        )
        .param("retryAfterSeconds", &retry_after_secs),
    )
    .with_headers(headers)
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
