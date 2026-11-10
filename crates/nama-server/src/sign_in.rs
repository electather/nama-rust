//! Sign-in attempts: one place that names the signing-in Device, lets the attempt past the sign-in
//! limits, checks its proof and has [`Users`] issue the session, in that order.

use std::time::Instant;

use crate::{
    device_names,
    rate_limits::{Blocked, SignInLimits},
    users::{self, NewDevice, SignedIn, Users},
};

/// Signs Devices in, counting every attempt as a guess until its proof checks out.
pub(crate) struct SignIn {
    users: Users,
    /// The recent failed attempts. `Setup` and `Reauthenticate` still count theirs here from their
    /// handlers, so all three share one limit per client IP.
    pub(crate) limits: SignInLimits,
}

/// Who a Device claims to sign in as, and what proves it. Not `Debug`, so the password can't end up
/// in a log.
pub(crate) enum Proof {
    /// A User's name, matched ignoring case, and their password.
    Password { name: String, password: String },
}

/// Why a sign-in attempt was refused or failed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// Too many attempts failed recently from the client IP or for the name.
    #[error(transparent)]
    Blocked(#[from] Blocked),
    /// The Device's name, the proof or the database.
    #[error(transparent)]
    Users(#[from] users::Error),
}

impl SignIn {
    /// Sign-in attempts against `users`, with no failures counted yet.
    pub(crate) fn new(users: Users) -> Self {
        Self {
            users,
            limits: SignInLimits::default(),
        }
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
        let attempt = self.limits.admit_at(device.ip_address, Some(&name), now)?;
        let (user, method) = self.users.verify(&name, &password).await?;
        self.limits.succeeded(&attempt);
        Ok(self.users.sign_in_as(user, Some(method), device).await?)
    }
}

/// `device` with its name checked, made from its `User-Agent` if it gave none.
pub(crate) fn named(device: NewDevice) -> Result<NewDevice, users::Error> {
    let name = if device.name.trim().is_empty() {
        device_names::from_user_agent(device.user_agent.as_deref())
    } else {
        device.name
    };
    let name = users::checked_name("device_name", "a Device's name", &name)?;
    Ok(NewDevice { name, ..device })
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, sync::Arc, time::Duration};

    use super::*;
    use crate::{db::Database, plugins::Registry};

    const PASSWORD: &str = "correct horse battery staple";

    const LAPTOP_IP: &str = "192.0.2.7";

    /// [`SignIn`] on a fresh database in `data_dir` with local-auth as the Authenticator and one
    /// User, Ada, whose password is [`PASSWORD`].
    async fn sign_in_with_ada(data_dir: &std::path::Path) -> SignIn {
        let db = Arc::new(Database::open(data_dir).await.unwrap());
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
            .unwrap();
        let users = Users::new(db, plugins);
        users
            .create_first_admin("Ada", PASSWORD, device("Setup", LAPTOP_IP))
            .await
            .unwrap();
        SignIn::new(users)
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
    fn retry_after_secs(refused: Result<SignedIn, Error>) -> Option<u64> {
        match refused {
            Err(Error::Blocked(blocked)) => Some(blocked.retry_after_secs()),
            _ => None,
        }
    }

    #[tokio::test]
    async fn the_11th_attempt_for_a_name_is_blocked_for_the_window_even_with_the_right_password() {
        let data_dir = tempfile::tempdir().unwrap();
        let sign_in = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        fail(&sign_in, "Ada", 10, now).await;

        let blocked = attempt(&sign_in, "aDA", PASSWORD, now).await;

        assert_eq!(retry_after_secs(blocked), Some(15 * 60));
    }

    #[tokio::test]
    async fn failures_for_one_name_dont_block_another_name() {
        let data_dir = tempfile::tempdir().unwrap();
        let sign_in = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        fail(&sign_in, "Grace", 10, now).await;

        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
    }

    #[tokio::test]
    async fn the_51st_attempt_from_an_address_is_blocked_whatever_the_name_but_not_from_another() {
        let data_dir = tempfile::tempdir().unwrap();
        let sign_in = sign_in_with_ada(data_dir.path()).await;
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
        let sign_in = sign_in_with_ada(data_dir.path()).await;
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
        let sign_in = sign_in_with_ada(data_dir.path()).await;
        let now = Instant::now();
        fail(&sign_in, "Ada", 9, now).await;
        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
        fail(&sign_in, "Ada", 9, now).await;

        attempt(&sign_in, "Ada", PASSWORD, now).await.unwrap();
    }

    #[tokio::test]
    async fn a_bad_device_name_is_refused_without_counting_as_a_guess() {
        let data_dir = tempfile::tempdir().unwrap();
        let sign_in = sign_in_with_ada(data_dir.path()).await;
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
}
