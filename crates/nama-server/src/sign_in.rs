//! Sign-in attempts, Setup included: one place that names the signing-in Device, lets the attempt
//! past the sign-in limits, checks its proof or the Setup Code and has [`Users`] issue the session,
//! in that order. Sign-in Codes go through it too. Reauthenticating follows the same order, giving
//! an existing Device Recent Authentication in place of a session, and signing out ends one.
//!
//! Guessable requests go through it (the crate's `AGENTS.md`, "Guessable requests").

use std::{net::IpAddr, time::Instant};

use crate::{
    codes::SetupCode,
    device_names,
    rate_limits::{Blocked, SignInCodeLimits, SignInLimits},
    sign_in_codes::{self, Delivery, Poll, SignInCodes, Started},
    users::{self, ApprovingDevice, Caller, NewDevice, Proved, SignedIn, Users},
};

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
}

/// Who a Device claims to be, and what proves it, when it signs in or reauthenticates. Not `Debug`,
/// so the password can't end up in a log.
pub(crate) enum Proof {
    /// A User's name, matched ignoring case, and their password.
    Password { name: String, password: String },
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
        }
    }

    /// Creates the first Admin, named `name` with `password`, for whoever gives `setup_code`, and
    /// signs `device` in as them, at `now`. Once the Server has Users every attempt is refused as
    /// already set up, whatever code it gives, so the code can't be probed then.
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
        // The Server only made no code because it started with Users.
        let server_code = self.setup_code.as_ref().ok_or(users::Error::AlreadySetUp)?;
        if self.users.any().await.map_err(users::Error::from)? {
            return Err(users::Error::AlreadySetUp.into());
        }
        if !server_code.matches(setup_code) {
            return Err(Error::WrongSetupCode);
        }
        self.limits.succeeded(&attempt);
        Ok(self
            .users
            .create_first_admin(name, password, device)
            .await?)
    }

    /// Signs `device` in as whoever `proof` proves it is, at `now`.
    pub(crate) async fn sign_in(
        &self,
        proof: Proof,
        device: NewDevice,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        // Input checks run before `admit`, so a mistake in them is never counted as a guess.
        let device = named(device)?;
        let Proof::Password { name, password } = proof;
        let attempt = self.limits.admit(device.ip_address, Some(&name), now)?;
        let (user, method) = self.users.verify(&attempt, &name, &password).await?;
        self.limits.succeeded(&attempt);
        Ok(self
            .users
            .sign_in_as(Proved::Method { user, method }, device)
            .await?)
    }

    /// Starts signing in, with a Sign-in Code, the Device `device_name` that sent `user_agent`, at
    /// `now`. `delivery` is kept for the poll that signs it in, unread.
    pub(crate) fn start_code(
        &self,
        device_name: String,
        user_agent: Option<&str>,
        delivery: Delivery,
        now: Instant,
    ) -> Result<Started, Error> {
        let device_name = checked_device_name(device_name, user_agent)?;
        Ok(self.codes.start(device_name, delivery, now)?)
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
    /// in from `ip_address` with `user_agent` if the code was approved. That uses the code up, so
    /// the poll's request source is a value its caller must have read already.
    pub(crate) async fn poll_code(
        &self,
        handle: &str,
        ip_address: IpAddr,
        user_agent: Option<String>,
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
            .sign_in_as(Proved::Code(approving), device)
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

    /// Gives `caller`'s Device Recent Authentication if `proof` proves it is their User, as asked
    /// from `ip` at `now`. A missing proof is refused with [`Error::ProofMissing`] and a Device
    /// found signed out once the proof checked out with [`users::Error::SignedOut`]; neither is a
    /// guess. A proof of another User is refused like a wrong password.
    pub(crate) async fn reauthenticate(
        &self,
        caller: &Caller,
        proof: Option<Proof>,
        ip: IpAddr,
        now: Instant,
    ) -> Result<(), Error> {
        let Proof::Password { name, password } = proof.ok_or(Error::ProofMissing)?;
        let attempt = self.limits.admit(ip, Some(&caller.user.name), now)?;
        if users::name_key(&name) != users::name_key(&caller.user.name) {
            return Err(users::Error::WrongPassword.into());
        }
        self.users
            .confirm_password(&attempt, &caller.user, &password)
            .await?;
        // Before recording, so a Device found signed out after the right password isn't a guess.
        self.limits.succeeded(&attempt);
        Ok(self.users.record_authentication(caller.device.id).await?)
    }

    /// Signs the Device `device` out, so its session stops working.
    pub(crate) async fn sign_out(&self, device: i64) -> Result<(), sqlx::Error> {
        self.users.sign_out(device).await
    }
}

/// `device` with its name checked, made from its `User-Agent` if it gave none.
fn named(device: NewDevice) -> Result<NewDevice, users::Error> {
    let name = checked_device_name(device.name, device.user_agent.as_deref())?;
    Ok(NewDevice { name, ..device })
}

/// `name` checked as a Device's, or made from `user_agent` if it is blank.
fn checked_device_name(name: String, user_agent: Option<&str>) -> Result<String, users::Error> {
    let name = if name.trim().is_empty() {
        device_names::from_user_agent(user_agent)
    } else {
        name
    };
    users::checked_name("device_name", "a Device's name", &name)
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, sync::Arc, time::Duration};

    use super::*;
    use crate::{db::Database, plugins::Registry, users::Permissions};

    const PASSWORD: &str = "correct horse battery staple";

    const LAPTOP_IP: &str = "192.0.2.7";

    /// [`SignIn`] on a fresh database in `data_dir`, with no Users yet and local-auth as the
    /// Authenticator, and the Setup Code it takes.
    async fn fresh_sign_in(data_dir: &std::path::Path) -> (SignIn, String) {
        let setup_code = SetupCode::generate().unwrap();
        let entered = setup_code.to_string();
        (
            SignIn::new(local_auth_users(data_dir).await, Some(setup_code)),
            entered,
        )
    }

    /// The Users kept in the database in `data_dir`, with local-auth as the Authenticator.
    async fn local_auth_users(data_dir: &std::path::Path) -> Users {
        let db = Arc::new(Database::open(data_dir).await.unwrap());
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
            .unwrap();
        Users::new(db, plugins)
    }

    /// [`SignIn`] on a fresh database in `data_dir`, set up with one User, Ada, whose password is
    /// [`PASSWORD`], and the Setup Code it took.
    async fn sign_in_with_ada(data_dir: &std::path::Path) -> (SignIn, String) {
        let (sign_in, setup_code) = fresh_sign_in(data_dir).await;
        set_up(&sign_in, &setup_code, "Ada", Instant::now())
            .await
            .unwrap();
        (sign_in, setup_code)
    }

    /// Sets up from [`LAPTOP_IP`] with `setup_code`, creating `name` with [`PASSWORD`], at `now`.
    async fn set_up(
        sign_in: &SignIn,
        setup_code: &str,
        name: &str,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        sign_in
            .setup(setup_code, name, PASSWORD, device("Laptop", LAPTOP_IP), now)
            .await
    }

    /// The Device `name`, signing in from `ip`.
    fn device(name: &str, ip: &str) -> NewDevice {
        NewDevice {
            name: name.to_owned(),
            ip_address: ip.parse::<IpAddr>().unwrap(),
            user_agent: None,
        }
    }

    fn password(name: &str, password: &str) -> Proof {
        Proof::Password {
            name: name.to_owned(),
            password: password.to_owned(),
        }
    }

    /// Signs in from [`LAPTOP_IP`] as `name` with `password` at `now`.
    async fn attempt(
        sign_in: &SignIn,
        name: &str,
        password_given: &str,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        sign_in
            .sign_in(
                password(name, password_given),
                device("Laptop", LAPTOP_IP),
                now,
            )
            .await
    }

    /// Fails to sign in as `name` with a wrong password `times` times at `now`.
    async fn fail(sign_in: &SignIn, name: &str, times: usize, now: Instant) {
        for _ in 0..times {
            let failed = attempt(sign_in, name, "not the password", now).await;
            assert!(
                matches!(failed, Err(Error::Users(users::Error::WrongNameOrPassword))),
                "{:?}",
                failed.map(|_| ())
            );
        }
    }

    /// How long `refused` says to wait, if it was blocked.
    fn retry_after_secs<T>(refused: Result<T, Error>) -> Option<u64> {
        match refused {
            Err(Error::Blocked(blocked)) => Some(blocked.retry_after_secs()),
            _ => None,
        }
    }

    #[tokio::test]
    async fn the_11th_attempt_for_a_name_is_blocked_for_the_window_even_with_the_right_password() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        fail(&sign_in, "Ada", 10, now).await;

        let blocked = attempt(&sign_in, "aDA", PASSWORD, now).await;

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn failures_for_one_name_dont_block_another_name() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        fail(&sign_in, "Grace", 10, now).await;

        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
    }

    #[tokio::test]
    async fn the_51st_attempt_from_an_address_is_blocked_whatever_the_name_but_not_from_another() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        for name in ["Grace", "Linus", "Alan", "Edsger", "Barbara"] {
            fail(&sign_in, name, 10, now).await;
        }

        let blocked = attempt(&sign_in, "Ada", PASSWORD, now).await;
        let elsewhere = sign_in
            .sign_in(
                password("Ada", PASSWORD),
                device("Phone", "198.51.100.1"),
                now,
            )
            .await;

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
        elsewhere.unwrap();
    }

    #[tokio::test]
    async fn a_blocked_name_is_let_through_again_once_its_failures_leave_the_window() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let start = Instant::now();
        fail(&sign_in, "Ada", 10, start).await;

        let blocked = attempt(&sign_in, "Ada", PASSWORD, start + Duration::from_mins(14)).await;
        let let_through = attempt(&sign_in, "Ada", PASSWORD, start + Duration::from_mins(15)).await;

        assert_eq!(retry_after_secs(blocked), Some(60));
        let_through.unwrap();
    }

    #[tokio::test]
    async fn a_successful_sign_in_forgets_earlier_failures_for_its_name() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        fail(&sign_in, "Ada", 9, now).await;
        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
        fail(&sign_in, "Ada", 9, now).await;

        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
    }

    #[tokio::test]
    async fn a_bad_device_name_is_refused_without_counting_as_a_guess() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let too_long = "D".repeat(65);
        for _ in 0..10 {
            let refused = sign_in
                .sign_in(
                    password("Ada", "not the password"),
                    device(&too_long, LAPTOP_IP),
                    now,
                )
                .await;
            assert!(
                matches!(
                    refused,
                    Err(Error::Users(users::Error::NameLength {
                        field: "device_name",
                        ..
                    }))
                ),
                "{:?}",
                refused.map(|_| ())
            );
        }

        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
    }

    /// Fails to set up from [`LAPTOP_IP`] with a wrong Setup Code `times` times at `now`.
    async fn fail_setup(sign_in: &SignIn, times: usize, now: Instant) {
        for _ in 0..times {
            let failed = set_up(sign_in, "AAAA-AAAA-AAAA", "Ada", now).await;
            assert!(
                matches!(failed, Err(Error::WrongSetupCode)),
                "{:?}",
                failed.map(|_| ())
            );
        }
    }

    #[tokio::test]
    async fn setup_with_the_right_code_creates_the_first_admin_signed_in_on_the_device() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;

        let signed_in = set_up(&sign_in, &setup_code, "Ada", Instant::now())
            .await
            .unwrap();

        assert_eq!(signed_in.caller.user.name, "Ada");
        assert_eq!(signed_in.caller.user.permissions, Permissions::ADMIN);
        assert_eq!(signed_in.caller.device.name, "Laptop");
    }

    #[tokio::test]
    async fn the_51st_setup_from_an_address_after_wrong_codes_is_blocked_even_with_the_right_code()
    {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;
        let now = Instant::now();
        fail_setup(&sign_in, 50, now).await;

        let blocked = set_up(&sign_in, &setup_code, "Ada", now).await;

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn the_51st_failure_from_an_address_mixing_sign_in_and_setup_is_blocked() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;
        let now = Instant::now();
        fail_setup(&sign_in, 40, now).await;
        fail(&sign_in, "Grace", 10, now).await;

        let blocked = set_up(&sign_in, &setup_code, "Ada", now).await;

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn once_the_server_has_users_setup_is_refused_as_already_set_up_whatever_the_code() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, setup_code) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();

        let right_code = set_up(&sign_in, &setup_code, "Grace", now).await;
        let wrong_code = set_up(&sign_in, "AAAA-AAAA-AAAA", "Grace", now).await;

        for refused in [right_code, wrong_code] {
            assert!(
                matches!(refused, Err(Error::Users(users::Error::AlreadySetUp))),
                "{:?}",
                refused.map(|_| ())
            );
        }
    }

    #[tokio::test]
    async fn setup_on_a_server_that_started_with_users_is_refused_as_already_set_up() {
        let data_dir = tempfile::tempdir().unwrap();
        drop(sign_in_with_ada(data_dir.path()).await);
        let restarted = SignIn::new(local_auth_users(data_dir.path()).await, None);

        let refused = set_up(&restarted, "AAAA-AAAA-AAAA", "Grace", Instant::now()).await;

        assert!(
            matches!(refused, Err(Error::Users(users::Error::AlreadySetUp))),
            "{:?}",
            refused.map(|_| ())
        );
    }

    #[tokio::test]
    async fn a_bad_admin_name_with_the_right_setup_code_is_refused_without_counting_as_a_guess() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, setup_code) = fresh_sign_in(data_dir.path()).await;
        let now = Instant::now();
        for _ in 0..50 {
            let refused = set_up(&sign_in, &setup_code, "   ", now).await;
            assert!(
                matches!(
                    refused,
                    Err(Error::Users(users::Error::NameLength { field: "name", .. }))
                ),
                "{:?}",
                refused.map(|_| ())
            );
        }

        set_up(&sign_in, &setup_code, "Ada", now).await.unwrap();
    }

    /// Ada's Device "Phone", signed in from [`LAPTOP_IP`] by a Sign-in Code approved from a Device
    /// signed in first, so "Phone" has no Recent Authentication yet.
    async fn ada_without_recent_authentication(sign_in: &SignIn) -> Caller {
        let laptop = ada_on_a_new_device(sign_in, Instant::now()).await;
        let phone = device("Phone", LAPTOP_IP);
        sign_in
            .users
            .sign_in_as(Proved::Code(ApprovingDevice::from(&laptop)), phone)
            .await
            .unwrap()
            .caller
    }

    /// Reauthenticates `caller` from [`LAPTOP_IP`] with `password_given` at `now`.
    async fn reauthenticate(
        sign_in: &SignIn,
        caller: &Caller,
        password_given: &str,
        now: Instant,
    ) -> Result<(), Error> {
        let ip = LAPTOP_IP.parse().unwrap();
        sign_in
            .reauthenticate(
                caller,
                Some(password(&caller.user.name, password_given)),
                ip,
                now,
            )
            .await
    }

    /// Fails to reauthenticate `caller` with a wrong password `times` times at `now`.
    async fn fail_reauthentication(sign_in: &SignIn, caller: &Caller, times: usize, now: Instant) {
        for _ in 0..times {
            let failed = reauthenticate(sign_in, caller, "not the password", now).await;
            assert!(
                matches!(failed, Err(Error::Users(users::Error::WrongPassword))),
                "{failed:?}"
            );
        }
    }

    #[tokio::test]
    async fn reauthenticating_with_the_right_password_gives_the_device_recent_authentication() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let ada = ada_without_recent_authentication(&sign_in).await;

        reauthenticate(&sign_in, &ada, PASSWORD, Instant::now())
            .await
            .unwrap();

        let devices = sign_in.users.devices(ada.user.id).await.unwrap().unwrap();
        let phone = devices.iter().find(|d| d.id == ada.device.id).unwrap();
        assert!(phone.authenticated_recently(phone.created_at), "{phone:?}");
    }

    #[tokio::test]
    async fn the_11th_failed_reauthentication_blocks_reauthenticating_and_signing_in_with_that_name()
     {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let ada = ada_without_recent_authentication(&sign_in).await;
        let now = Instant::now();
        fail_reauthentication(&sign_in, &ada, 10, now).await;

        let reauthenticating = reauthenticate(&sign_in, &ada, PASSWORD, now).await;
        let signing_in = attempt(&sign_in, "Ada", PASSWORD, now).await;

        assert!(
            matches!(reauthenticating, Err(Error::Blocked(_))),
            "{reauthenticating:?}"
        );
        assert_eq!(retry_after_secs(signing_in), Some(15 * 60));
    }

    #[tokio::test]
    async fn the_51st_failure_from_an_address_mixing_sign_in_and_reauthentication_is_blocked() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let ada = ada_without_recent_authentication(&sign_in).await;
        let now = Instant::now();
        for name in ["Grace", "Linus", "Alan", "Edsger"] {
            fail(&sign_in, name, 10, now).await;
        }
        fail_reauthentication(&sign_in, &ada, 10, now).await;

        let blocked = attempt(&sign_in, "Barbara", "not the password", now).await;

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn a_successful_reauthentication_forgets_earlier_failures_for_its_name() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let ada = ada_without_recent_authentication(&sign_in).await;
        let now = Instant::now();
        fail_reauthentication(&sign_in, &ada, 9, now).await;
        reauthenticate(&sign_in, &ada, PASSWORD, now).await.unwrap();
        fail_reauthentication(&sign_in, &ada, 9, now).await;

        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
    }

    #[tokio::test]
    async fn the_right_password_on_a_signed_out_device_is_refused_without_counting_as_a_guess() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let ada = ada_without_recent_authentication(&sign_in).await;
        sign_in.sign_out(ada.device.id).await.unwrap();
        let now = Instant::now();
        for _ in 0..10 {
            let refused = reauthenticate(&sign_in, &ada, PASSWORD, now).await;
            assert!(
                matches!(refused, Err(Error::Users(users::Error::SignedOut))),
                "{refused:?}"
            );
        }

        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
    }

    #[tokio::test]
    async fn reauthenticating_with_another_users_name_and_password_is_refused_as_a_wrong_password()
    {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let ada = ada_without_recent_authentication(&sign_in).await;
        sign_in
            .users
            .create(&ada.user, "Grace", PASSWORD)
            .await
            .unwrap();
        let ip = LAPTOP_IP.parse().unwrap();

        let refused = sign_in
            .reauthenticate(&ada, Some(password("Grace", PASSWORD)), ip, Instant::now())
            .await;

        assert!(
            matches!(refused, Err(Error::Users(users::Error::WrongPassword))),
            "{refused:?}"
        );
    }

    #[tokio::test]
    async fn a_reauthentication_without_a_proof_is_refused_without_counting_as_a_guess() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let ada = ada_without_recent_authentication(&sign_in).await;
        let ip = LAPTOP_IP.parse().unwrap();
        let now = Instant::now();
        for _ in 0..50 {
            let refused = sign_in.reauthenticate(&ada, None, ip, now).await;
            assert!(matches!(refused, Err(Error::ProofMissing)), "{refused:?}");
        }

        reauthenticate(&sign_in, &ada, PASSWORD, now).await.unwrap();
    }

    /// Ada, signed in at `now` on a new Device.
    async fn ada_on_a_new_device(sign_in: &SignIn, now: Instant) -> Caller {
        attempt(sign_in, "Ada", PASSWORD, now).await.unwrap().caller
    }

    /// Starts a Sign-in Code at `now` for the Device "Living room TV", wanting a token.
    fn start_code(sign_in: &SignIn, now: Instant) -> Started {
        sign_in
            .start_code("Living room TV".to_owned(), None, Delivery::Token, now)
            .unwrap()
    }

    #[tokio::test]
    async fn a_code_started_without_a_name_is_named_after_the_devices_browser_and_os() {
        const MAC_CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
            AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;

        let started = sign_in
            .start_code(" ".to_owned(), Some(MAC_CHROME), Delivery::Token, now)
            .unwrap();
        let device_name = sign_in.look_up_code(&ada, &started.code, now).unwrap();

        assert_eq!(device_name, "Chrome on macOS");
    }

    const TV_IP: &str = "198.51.100.9";

    /// Polls `handle` at `now` from the Device that started it.
    async fn poll_code(sign_in: &SignIn, handle: &str, now: Instant) -> Result<Polled, Error> {
        let ip_address = TV_IP.parse().unwrap();
        sign_in.poll_code(handle, ip_address, None, now).await
    }

    #[tokio::test]
    async fn an_approved_code_signs_its_device_in_as_the_approver_once() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;
        let started = start_code(&sign_in, now);

        sign_in.approve_code(&ada, &started.code, now).unwrap();
        let first = poll_code(&sign_in, &started.handle, now).await;
        let again = poll_code(&sign_in, &started.handle, now).await;

        let Ok(Polled::SignedIn {
            signed_in,
            delivery,
        }) = first
        else {
            panic!("not signed in");
        };
        let tv = signed_in.caller;
        assert_eq!(
            (tv.user.id, tv.device.name.as_str(), delivery),
            (ada.user.id, "Living room TV", Delivery::Token)
        );
        assert!(
            matches!(
                again,
                Err(Error::Code(sign_in_codes::Error::HandleNotFound))
            ),
            "the code signed in twice"
        );
    }

    /// Asserts that polling `handle` again at `now` finds no sign-in waiting.
    async fn assert_used_up(sign_in: &SignIn, handle: &str, now: Instant) {
        let again = poll_code(sign_in, handle, now).await;
        assert!(
            matches!(
                again,
                Err(Error::Code(sign_in_codes::Error::HandleNotFound))
            ),
            "the code wasn't used up"
        );
    }

    #[tokio::test]
    async fn a_code_whose_approving_user_was_deleted_before_the_poll_is_refused_and_used_up() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;
        let grace = sign_in.users.create(&ada.user, "Grace", PASSWORD).await;
        let grace = grace.unwrap();
        let graces_laptop = attempt(&sign_in, "Grace", PASSWORD, now).await.unwrap();
        let started = start_code(&sign_in, now);
        sign_in
            .approve_code(&graces_laptop.caller, &started.code, now)
            .unwrap();
        sign_in.users.delete(&ada.user, grace).await.unwrap();

        let polled = poll_code(&sign_in, &started.handle, now).await;

        assert!(
            matches!(
                polled,
                Err(Error::Users(users::Error::ApprovingUserDeleted))
            ),
            "{:?}",
            polled.as_ref().err()
        );
        assert_used_up(&sign_in, &started.handle, now).await;
    }

    #[tokio::test]
    async fn a_code_whose_approving_device_signed_out_before_the_poll_is_refused_and_used_up() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;
        let started = start_code(&sign_in, now);
        sign_in.approve_code(&ada, &started.code, now).unwrap();
        sign_in.sign_out(ada.device.id).await.unwrap();

        let polled = poll_code(&sign_in, &started.handle, now).await;

        assert!(
            matches!(
                polled,
                Err(Error::Users(users::Error::ApprovingDeviceSignedOut))
            ),
            "{:?}",
            polled.as_ref().err()
        );
        assert_used_up(&sign_in, &started.handle, now).await;
    }

    #[tokio::test]
    async fn a_code_polls_waiting_until_approved_and_expired_once_its_lifetime_is_over() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let started_at = Instant::now();
        let started = start_code(&sign_in, started_at);

        let waiting = poll_code(&sign_in, &started.handle, started_at).await;
        let expired = poll_code(
            &sign_in,
            &started.handle,
            started_at + Duration::from_mins(10),
        )
        .await;

        assert!(matches!(waiting, Ok(Polled::Waiting)));
        assert!(matches!(expired, Ok(Polled::Expired)));
    }

    #[tokio::test]
    async fn refusals_of_a_code_never_show_the_code_or_its_handle() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;
        let started = start_code(&sign_in, now);
        sign_in.approve_code(&ada, &started.code, now).unwrap();

        let approved_again = sign_in.approve_code(&ada, &started.code, now).err();
        poll_code(&sign_in, &started.handle, now).await.unwrap();
        let looked_up_once_used = sign_in.look_up_code(&ada, &started.code, now).err();
        let polled_again = poll_code(&sign_in, &started.handle, now).await.err();

        let secrets = [
            started.code.clone(),
            started.code.replace('-', ""),
            started.handle.clone(),
        ];
        for refused in [approved_again, looked_up_once_used, polled_again] {
            let refused = refused.expect("refused");
            let shown = format!("{refused} {refused:?}");
            assert!(
                secrets
                    .iter()
                    .all(|secret| !shown.contains(secret.as_str())),
                "{shown}"
            );
        }
    }

    /// Fails `times` times to look up `code`, unknown or expired, as `caller` at `now`.
    fn fail_look_up(sign_in: &SignIn, caller: &Caller, code: &str, times: usize, now: Instant) {
        for _ in 0..times {
            let refused = sign_in.look_up_code(caller, code, now);
            assert!(
                matches!(refused, Err(Error::Code(sign_in_codes::Error::NotFound))),
                "{refused:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_11th_failed_code_lookup_from_a_device_is_blocked_even_for_a_real_code() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;
        let started = start_code(&sign_in, now);
        fail_look_up(&sign_in, &ada, "AAAA-AAAA", 10, now);

        let blocked = sign_in.look_up_code(&ada, &started.code, now);

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn failed_code_lookups_and_approvals_count_towards_the_same_limit() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;
        let started = start_code(&sign_in, now);
        fail_look_up(&sign_in, &ada, "AAAA-AAAA", 5, now);
        for _ in 0..5 {
            let refused = sign_in.approve_code(&ada, "AAAA-AAAA", now);
            assert!(matches!(
                refused,
                Err(Error::Code(sign_in_codes::Error::NotFound))
            ));
        }

        let blocked = sign_in.approve_code(&ada, &started.code, now);

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn an_expired_code_counts_against_the_device_like_an_unknown_one() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let start = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, start).await;
        let expired = start_code(&sign_in, start);
        let expired_at = start + Duration::from_mins(10);
        let started = start_code(&sign_in, expired_at);
        fail_look_up(&sign_in, &ada, &expired.code, 10, expired_at);

        let blocked = sign_in.look_up_code(&ada, &started.code, expired_at);

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn failed_code_lookups_from_one_device_dont_block_another_device() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let laptop = ada_on_a_new_device(&sign_in, now).await;
        let phone = ada_on_a_new_device(&sign_in, now).await;
        let started = start_code(&sign_in, now);
        fail_look_up(&sign_in, &laptop, "AAAA-AAAA", 10, now);

        sign_in.approve_code(&phone, &started.code, now).unwrap();
    }

    #[tokio::test]
    async fn a_blocked_device_is_let_through_again_once_its_failures_leave_the_window() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let start = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, start).await;
        fail_look_up(&sign_in, &ada, "AAAA-AAAA", 10, start);
        let later = start + Duration::from_mins(15);
        let started = start_code(&sign_in, later);

        let looked_up = sign_in.look_up_code(&ada, &started.code, later);

        assert_eq!(looked_up.unwrap(), "Living room TV");
    }

    #[tokio::test]
    async fn using_a_real_code_never_counts_against_the_device() {
        let data_dir = tempfile::tempdir().unwrap();
        let (sign_in, _) = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        let ada = ada_on_a_new_device(&sign_in, now).await;
        let approved = start_code(&sign_in, now);
        for _ in 0..10 {
            sign_in.look_up_code(&ada, &approved.code, now).unwrap();
        }
        sign_in.approve_code(&ada, &approved.code, now).unwrap();
        let refused_again = (0..10).flat_map(|_| {
            [
                sign_in.look_up_code(&ada, &approved.code, now).map(|_| ()),
                sign_in.approve_code(&ada, &approved.code, now),
            ]
        });
        for refused in refused_again {
            assert!(matches!(
                refused,
                Err(Error::Code(sign_in_codes::Error::AlreadyApproved))
            ));
        }
        let started = start_code(&sign_in, now);

        sign_in.look_up_code(&ada, &started.code, now).unwrap();
    }
}
