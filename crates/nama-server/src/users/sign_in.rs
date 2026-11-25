//! Sign-in attempts, Setup included: one place that names the signing-in Device, lets the attempt
//! past the sign-in limits, checks its proof or the Setup Code and has [`Users`] issue the session,
//! in that order. Sign-in Codes and accepting Invites go through it too. Reauthenticating follows
//! the same order, giving an existing Device Recent Authentication in place of a session, and
//! signing out ends one.
//!
//! Guessable requests go through it (the crate's `AGENTS.md`, "Guessable requests").

use std::{net::IpAddr, time::Instant};

use super::{
    self as users, ApprovingDevice, Caller, NewDevice, Proved, SignedIn, Users,
    codes::SetupCode,
    device_names,
    passwords::is_switch_pin,
    rate_limits::{
        Blocked, SignInCodeLimits, SignInCodeStartLimits, SignInLimits, SwitchPinTries,
        WRONG_SWITCH_PINS,
    },
    sign_in_codes::{self, Delivery, Poll, SignInCodes, Started},
};
use crate::link;

/// Signs Devices in, with a proof or a Sign-in Code, under the sign-in limits.
pub(crate) struct SignIn {
    users: Users,
    /// What `setup` must be given. `None` once the Server started with Users.
    setup_code: Option<SetupCode>,
    /// The recent failed attempts of Setup, signing in and Reauthenticate alike, so they share one
    /// limit per client IP.
    limits: SignInLimits,
    codes: SignInCodes,
    /// The recent failed Sign-in Code lookups and approvals, which slow guessing codes down.
    code_limits: SignInCodeLimits,
    /// The recent Sign-in Code starts, which keep one client IP from filling `codes`.
    code_start_limits: SignInCodeStartLimits,
    switch_pin_tries: SwitchPinTries,
}

/// Who a Device claims to be, and what proves it, when it signs in or reauthenticates. Not `Debug`,
/// so the password or handle can't end up in a log.
pub(crate) enum Proof {
    /// A User's name, matched ignoring case, and their password.
    Password { name: String, password: String },
    /// The secret handle of a sign-in with an Account, and the one-time code the service sent the
    /// person back with: the User Linked to the Account that approved it (ADR 0092).
    Account { handle: String, code: String },
}

/// Where a Sign-in Code polled with its handle has got to. Not `Debug`: it may hold a session
/// token.
pub(crate) enum Polled {
    /// Not approved yet.
    Waiting,
    /// Not approved, or not polled once approved, within the code's lifetime.
    Expired,
    /// Approved, so the polling Device is signed in as the approver, to be given its session by
    /// `delivery`, as it asked when it started the code.
    SignedIn {
        signed_in: SignedIn,
        delivery: Delivery,
    },
}

/// Why a sign-in attempt was refused or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// Too many attempts failed recently from the client IP or for the name.
    #[error(transparent)]
    Blocked(#[from] Blocked),
    /// Too many Sign-in Codes were started recently from the client IP. Not [`Error::Blocked`]:
    /// no attempt failed.
    #[error(
        "too many Sign-in Codes were started from your address; try again in {} seconds",
        .0.retry_after_secs()
    )]
    TooManyCodeStarts(Blocked),
    /// Setup was given a code other than the Server's Setup Code.
    #[error("the Setup Code is wrong")]
    WrongSetupCode,
    /// The Device's name, the proof, the new Admin, a Server already set up, a Device signed out
    /// meanwhile or the database.
    #[error(transparent)]
    Users(#[from] users::Error),
    /// A Sign-in Code or poll handle.
    #[error(transparent)]
    Code(#[from] sign_in_codes::Error),
    /// Reauthenticating gave no proof of who the caller is.
    #[error("the proof of who you are is missing")]
    ProofMissing,
    /// Reauthenticating with an Account, the Account that approved isn't the caller's.
    #[error("the Account that approved isn't the one Linked to you")]
    WrongAccount,
    /// The sign-in with an Account whose handle came in the request field `field` wasn't
    /// approved, or expired.
    #[error("the sign-in wasn't approved on the Hub, or it expired; start again")]
    AccountSignInNotFound {
        /// The request field it is about, as clients name it.
        field: &'static str,
    },
    /// Switching to a User with a Switch PIN gave none.
    #[error("that User has a Switch PIN; enter it")]
    SwitchPinMissing,
    /// The Switch PIN isn't the User's. After [`WRONG_SWITCH_PINS`] in a row, that User was
    /// signed out of the Device.
    #[error(
        "the Switch PIN is wrong; too many wrong ones in a row sign that User out of this Device"
    )]
    WrongSwitchPin {
        /// How many more wrong ones would sign the User out, 0 once they were.
        tries_left: u32,
    },
    /// The Plugin that links the Server couldn't finish a sign-in with an Account.
    #[error(transparent)]
    Link(#[from] link::Error),
}

impl SignIn {
    /// Sign-in attempts against `users`, with no failures counted yet, that set up the Server with
    /// `setup_code` while it has no Users.
    pub(crate) fn new(users: Users, setup_code: Option<SetupCode>) -> Self {
        Self {
            users,
            setup_code,
            limits: SignInLimits::default(),
            codes: SignInCodes::default(),
            code_limits: SignInCodeLimits::default(),
            code_start_limits: SignInCodeStartLimits::default(),
            switch_pin_tries: SwitchPinTries::default(),
        }
    }

    /// Creates the first Admin, named `name` with `password`, for whoever gives `setup_code`, and
    /// signs `device` in as them, at `now`. Once the Server has Users every attempt is refused as
    /// already set up, whatever code it gives, so the code can't be probed then. Such a refusal
    /// isn't counted toward the sign-in limit.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is its own fact about the attempt, and a struct would only rename them"
    )]
    pub(crate) async fn setup(
        &self,
        setup_code: &str,
        name: &str,
        password: &str,
        device: NewDevice,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        let device = named(device)?;
        let attempt = self.limits.admit(device.ip_address, None, now)?;
        let server_code = match &self.setup_code {
            Some(code) if !self.users.any().await.map_err(users::Error::from)? => code,
            // The Server has Users, or started with them and so made no code. Whether a Server is
            // set up is public (`ServerInfo`), so asking again is no guess.
            _ => {
                self.limits.succeeded(&attempt);
                return Err(users::Error::AlreadySetUp.into());
            }
        };
        if !server_code.matches(setup_code) {
            return Err(Error::WrongSetupCode);
        }
        self.limits.succeeded(&attempt);
        Ok(self
            .users
            .create_first_admin(name, password, device)
            .await?)
    }

    /// Signs `device` in as whoever `proof` proves it is, at `now`, adding them to it if it holds
    /// the `session` of a Device already signed in. An Account's proof isn't counted: the Hub
    /// checked who it is, and its handle is a secret.
    pub(crate) async fn sign_in(
        &self,
        proof: Proof,
        device: NewDevice,
        session: Option<&str>,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        // Input checks run before `admit`, so a mistake in them is never counted as a guess.
        let device = named(device)?;
        let proved = match proof {
            Proof::Password { name, password } => {
                let attempt = self.limits.admit(device.ip_address, Some(&name), now)?;
                let (user, method) = self.users.verify(&attempt, &name, &password).await?;
                self.limits.succeeded(&attempt);
                Proved::Method { user, method }
            }
            Proof::Account { handle, code } => {
                Proved::Account(self.approved(&handle, &code, "handle").await?)
            }
        };
        Ok(self.users.sign_in_as(proved, device, session).await?)
    }

    /// Creates a User named `name` with `password` from the Invite whose link carries `token`, and
    /// signs `device` in as them, at `now`. A token of no working Invite counts as a failed attempt
    /// from the client IP; a name or password refused once the token checked out doesn't.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is its own fact about the attempt, and a struct would only rename them"
    )]
    pub(crate) async fn accept_invite(
        &self,
        token: &str,
        name: &str,
        password: &str,
        device: NewDevice,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        let device = named(device)?;
        let attempt = self.limits.admit(device.ip_address, None, now)?;
        self.users.ensure_invite_works(&attempt, token).await?;
        self.limits.succeeded(&attempt);
        Ok(self
            .users
            .accept_invite(token, name, password, device)
            .await?)
    }

    /// Starts signing in `device` with a Sign-in Code at `now`, unless its IP started too many
    /// recently. Of `device`, only its name is kept; `delivery` is kept, unread, for the poll that
    /// signs it in.
    pub(crate) fn start_code(
        &self,
        device: NewDevice,
        delivery: Delivery,
        now: Instant,
    ) -> Result<Started, Error> {
        // Before `admit`, so a start refused for its name isn't counted.
        let device = named(device)?;
        self.code_start_limits
            .admit(device.ip_address, now)
            .map_err(Error::TooManyCodeStarts)?;
        Ok(self.codes.start(device.name, delivery, now)?)
    }

    /// The name of the Device that the code `entered` would sign in, looked up by `caller` at
    /// `now`.
    pub(crate) fn look_up_code(
        &self,
        caller: &Caller,
        entered: &str,
        now: Instant,
    ) -> Result<String, Error> {
        self.counting_unknown_codes(caller, now, || self.codes.look_up(entered, now))
    }

    /// Approves the code `entered` at `now`, so its Device signs in as `caller`'s User.
    pub(crate) fn approve_code(
        &self,
        caller: &Caller,
        entered: &str,
        now: Instant,
    ) -> Result<(), Error> {
        self.counting_unknown_codes(caller, now, || {
            self.codes
                .approve(entered, ApprovingDevice::from(caller), now)
        })
    }

    /// Where the Sign-in Code polled with `handle` at `now` has got to, signing the polling Device
    /// in from `ip_address` with `user_agent` if the code was approved, as [`SignIn::sign_in`]
    /// does with its `session`. That uses the code up, so the poll's request source is a value its
    /// caller must have read already.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is its own fact about the poll, and a struct would only rename them"
    )]
    pub(crate) async fn poll_code(
        &self,
        handle: &str,
        ip_address: IpAddr,
        user_agent: Option<String>,
        session: Option<&str>,
        now: Instant,
    ) -> Result<Polled, Error> {
        let (approving, device_name, delivery) = match self.codes.poll(handle, now)? {
            Poll::Waiting => return Ok(Polled::Waiting),
            Poll::Expired => return Ok(Polled::Expired),
            Poll::Approved {
                approving,
                device_name,
                delivery,
            } => (approving, device_name, delivery),
        };
        let device = NewDevice {
            name: device_name,
            ip_address,
            user_agent,
        };
        let signed_in = self
            .users
            .sign_in_as(Proved::Code(approving), device, session)
            .await?;
        Ok(Polled::SignedIn {
            signed_in,
            delivery,
        })
    }

    /// Runs `use_code` for `caller` at `now`, unless their Device has failed too often lately. Only
    /// a code that is unknown or expired counts as a failure: that is what a guess gets.
    fn counting_unknown_codes<T>(
        &self,
        caller: &Caller,
        now: Instant,
        use_code: impl FnOnce() -> Result<T, sign_in_codes::Error>,
    ) -> Result<T, Error> {
        let attempt = self.code_limits.admit(caller.device.id, now)?;
        let used = use_code();
        if !matches!(used, Err(sign_in_codes::Error::NotFound)) {
            self.code_limits.wasnt_a_guess(&attempt);
        }
        Ok(used?)
    }

    /// Gives `caller`'s User Recent Authentication on their Device if `proof` proves who they are,
    /// as asked from `ip` at `now`. A missing proof is refused with [`Error::ProofMissing`] and a
    /// User found signed out of the Device once the proof checked out with
    /// [`users::Error::SignedOut`]; neither is a guess. A proof of another User is refused like a
    /// wrong password, or as [`Error::WrongAccount`]. The Hub is asked about an Account's only once
    /// the attempt is let through, and its failure counts as a guess.
    pub(crate) async fn reauthenticate(
        &self,
        caller: &Caller,
        proof: Option<Proof>,
        ip: IpAddr,
        now: Instant,
    ) -> Result<(), Error> {
        let proof = proof.ok_or(Error::ProofMissing)?;
        let attempt = self.limits.admit(ip, Some(&caller.user.name), now)?;
        match proof {
            Proof::Password { name, password } => {
                if users::name_key(&name) != users::name_key(&caller.user.name) {
                    return Err(users::Error::WrongPassword.into());
                }
                self.users
                    .confirm_password(&attempt, &caller.user, &password)
                    .await?;
            }
            // A Hub that can't be asked counts too: un-counting it would forget the name's failures.
            Proof::Account { handle, code } => {
                if self
                    .approved(&handle, &code, "account_sign_in_handle")
                    .await?
                    != caller.user.id
                {
                    return Err(Error::WrongAccount);
                }
            }
        }
        // Before recording, so a Device found signed out after the proof checked out isn't a guess.
        self.limits.succeeded(&attempt);
        Ok(self
            .users
            .record_authentication(caller.device.id, caller.user.id)
            .await?)
    }

    /// The id of the User whose Account approved the sign-in with an Account `handle` stands for,
    /// finished with the service's one-time `code`. A refusal names `field`, the request field
    /// `handle` came in.
    async fn approved(&self, handle: &str, code: &str, field: &'static str) -> Result<i64, Error> {
        (self
            .users
            .linking
            .finish_account_sign_in(handle, code)
            .await?)
            .ok_or(Error::AccountSignInNotFound { field })
    }

    /// Makes the Device `device` act as the User `user`, one of those signed in to it, once
    /// `switch_pin` is theirs, if they set one (ADR 0106). The last of [`WRONG_SWITCH_PINS`] wrong
    /// ones in a row signs them out of the Device instead. A missing one is no guess.
    pub(crate) async fn switch(
        &self,
        device: i64,
        user: i64,
        switch_pin: &str,
    ) -> Result<(), Error> {
        // Read before counting: a User not on the Device, or with no Switch PIN, is no guess.
        if let Some(stored) = self.users.switch_pin_on(device, user).await? {
            self.check_switch_pin(device, user, switch_pin, stored)
                .await?;
        }
        Ok(self.users.switch(device, user).await?)
    }

    /// Fails unless `switch_pin` is the one `stored` hashes, the Switch PIN of the User `user`,
    /// counting it toward signing them out of the Device `device` as [`SignIn::switch`] says.
    async fn check_switch_pin(
        &self,
        device: i64,
        user: i64,
        switch_pin: &str,
        stored: String,
    ) -> Result<(), Error> {
        if switch_pin.is_empty() {
            return Err(Error::SwitchPinMissing);
        }
        let tried = (self.switch_pin_tries.admit(device, user))
            .ok_or(Error::WrongSwitchPin { tries_left: 0 })?;
        match is_switch_pin(&tried, switch_pin, stored).await {
            Ok(true) => {
                self.switch_pin_tries.forget(device, user);
                return Ok(());
            }
            Ok(false) => {}
            // A Server failure is no guess, and left counted, five of them would lock switching.
            Err(err) => {
                self.switch_pin_tries.uncount(device, user);
                return Err(err.into());
            }
        }
        let tries_left = WRONG_SWITCH_PINS - tried.in_a_row;
        if tries_left == 0 {
            // The full count refuses other tries until the sign-out lands, and is forgotten even
            // if it fails, so it can't leave switching locked.
            let signed_out = self.users.sign_out_of(device, user).await;
            self.switch_pin_tries.forget(device, user);
            signed_out?;
        }
        Err(Error::WrongSwitchPin { tries_left })
    }

    /// Signs the Device `device` out, with every User on it, so its session stops working.
    pub(crate) async fn sign_out(&self, device: i64) -> Result<(), sqlx::Error> {
        self.users.sign_out(device).await
    }
}

/// `device` with its name checked, made from its `User-Agent` if it gave none.
fn named(device: NewDevice) -> Result<NewDevice, users::Error> {
    let name = if device.name.trim().is_empty() {
        device_names::from_user_agent(device.user_agent.as_deref())
    } else {
        device.name
    };
    let name = users::checked_name("device_name", "a Device's name", &name)?;
    Ok(NewDevice { name, ..device })
}

#[cfg(test)]
#[path = "sign_in_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "setup_tests.rs"]
mod setup_tests;
