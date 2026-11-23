//! Signing in with a passkey, and the passkeys an Account adds and removes (ADR 0100 for which
//! ways to sign in an Account must keep).

use std::{ops::RangeInclusive, time::Instant};

use uuid::Uuid;

use super::{
    Account, AccountId, AccountSession, Accounts, Caller, Error, SignedIn, sessions,
    webauthn::{Ceremony, RelyingParty, public_key},
};

/// How many characters a passkey's name may have, once trimmed.
pub(crate) const PASSKEY_NAME_LENGTH: RangeInclusive<usize> = 1..=64;

/// One of an Account's passkeys.
#[derive(Debug)]
pub(crate) struct Passkey {
    pub(crate) id: Uuid,
    pub(crate) name: String,
}

/// What a browser needs to sign in with a passkey, or to add one.
#[derive(Debug)]
pub(crate) struct Challenge {
    pub(crate) bytes: [u8; 32],
    /// The relying party's id: the Hub's public host.
    pub(crate) rp_id: String,
}

/// What a browser needs to add a passkey to an Account.
#[derive(Debug)]
pub(crate) struct Adding {
    pub(crate) challenge: Challenge,
    /// WebAuthn's user handle: the Account's id.
    pub(crate) user_id: [u8; 16],
    /// The Account's passkeys, which the browser shouldn't add again.
    pub(crate) exclude_credential_ids: Vec<Vec<u8>>,
}

/// A browser's answer to a sign-in challenge, its `AuthenticatorAssertionResponse`.
#[derive(Debug)]
pub(crate) struct Assertion<'a> {
    pub(crate) credential_id: &'a [u8],
    pub(crate) client_data_json: &'a [u8],
    pub(crate) authenticator_data: &'a [u8],
    pub(crate) signature: &'a [u8],
    /// The Account's id, which a passkey found without an email always gives (§7.2, step 6).
    pub(crate) user_handle: &'a [u8],
}

/// A passkey a browser just created, its `AuthenticatorAttestationResponse`, and its name.
#[derive(Debug)]
pub(crate) struct NewPasskey<'a> {
    pub(crate) name: &'a str,
    pub(crate) credential_id: &'a [u8],
    pub(crate) client_data_json: &'a [u8],
    /// A DER `SubjectPublicKeyInfo`.
    pub(crate) public_key: &'a [u8],
    /// A COSE algorithm id.
    pub(crate) algorithm: i32,
}

impl Accounts {
    /// A challenge for `caller` to sign in with any Account's passkey.
    pub(crate) fn start_passkey_sign_in(&self, caller: Caller) -> Result<Challenge, Error> {
        self.challenge(Ceremony::SignIn, caller)
    }

    /// Signs in the Account whose passkey made `assertion` at `now`, in place of `replacing`, the
    /// browser's session, if any.
    pub(crate) async fn sign_in_with_passkey(
        &self,
        assertion: &Assertion<'_>,
        replacing: Option<&AccountSession>,
        now: Instant,
    ) -> Result<SignedIn, Error> {
        let relying_party = self.relying_party()?;
        if !relying_party.answers(assertion.client_data_json, Ceremony::SignIn, now) {
            return Err(Error::PasskeyNotAccepted);
        }
        let mut tx = self.db.begin().await?;
        // The share lock holds a Sign-in Link's takeover, which deletes the passkey, until this
        // session exists for it to end too (ADR 0100).
        let found = sqlx::query!(
            "SELECT account_passkeys.algorithm, account_passkeys.public_key,
                    accounts.id AS account_id, accounts.email
             FROM account_passkeys JOIN accounts ON accounts.id = account_passkeys.account_id
             WHERE credential_id = $1
             FOR SHARE OF account_passkeys",
            assertion.credential_id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::PasskeyNotAccepted)?;
        let key =
            public_key(&found.public_key, found.algorithm).ok_or(Error::PasskeyNotAccepted)?;
        if assertion.user_handle != found.account_id.as_bytes()
            || !relying_party.signed(
                &key,
                assertion.authenticator_data,
                assertion.client_data_json,
                assertion.signature,
            )
        {
            return Err(Error::PasskeyNotAccepted);
        }
        let account = Account {
            id: AccountId(found.account_id),
            email: found.email,
        };
        if let Some(session) = replacing {
            sessions::end(&mut *tx, session).await?;
        }
        let token = sessions::start(&mut tx, account.id).await?;
        tx.commit().await?;
        Ok(SignedIn { account, token })
    }

    /// A challenge for `caller` to add a passkey to `session`'s Account.
    pub(crate) async fn start_adding_passkey(
        &self,
        session: &AccountSession,
        caller: Caller,
    ) -> Result<Adding, Error> {
        require_recent(session)?;
        let account = session.account.id;
        let challenge = self.challenge(Ceremony::Add(account), caller)?;
        let exclude_credential_ids = sqlx::query_scalar!(
            "SELECT credential_id FROM account_passkeys WHERE account_id = $1",
            account.0,
        )
        .fetch_all(&self.db)
        .await?;
        Ok(Adding {
            challenge,
            user_id: account.0.into_bytes(),
            exclude_credential_ids,
        })
    }

    /// Adds `passkey`, which a browser created at `now`, to `session`'s Account.
    pub(crate) async fn add_passkey(
        &self,
        session: &AccountSession,
        passkey: &NewPasskey<'_>,
        now: Instant,
    ) -> Result<Passkey, Error> {
        require_recent(session)?;
        let relying_party = self.relying_party()?;
        // Before the challenge is used up, so the same passkey can be sent again with a better name.
        let name = passkey.name.trim();
        if !PASSKEY_NAME_LENGTH.contains(&name.chars().count()) {
            return Err(Error::PasskeyNameLength);
        }
        let account = session.account.id;
        if !relying_party.answers(passkey.client_data_json, Ceremony::Add(account), now)
            || public_key(passkey.public_key, passkey.algorithm).is_none()
        {
            return Err(Error::PasskeyNotAccepted);
        }
        let mut tx = self.db.begin().await?;
        // Under the row lock, so a Sign-in Link's takeover, which removes passkeys, can't miss
        // this one (ADR 0100).
        if !sessions::lock_while_recent(&mut tx, session).await? {
            return Err(Error::RecentAuthenticationRequired);
        }
        let id = sqlx::query_scalar!(
            "INSERT INTO account_passkeys (account_id, credential_id, name, algorithm, public_key)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (credential_id) DO NOTHING RETURNING id",
            account.0,
            passkey.credential_id,
            name,
            passkey.algorithm,
            passkey.public_key,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::PasskeyNotAccepted)?;
        tx.commit().await?;
        Ok(Passkey {
            id,
            name: name.to_owned(),
        })
    }

    /// `account`'s passkeys, oldest first.
    pub(crate) async fn passkeys(&self, account: AccountId) -> Result<Vec<Passkey>, sqlx::Error> {
        sqlx::query_as!(
            Passkey,
            "SELECT id, name FROM account_passkeys WHERE account_id = $1 ORDER BY id",
            account.0,
        )
        .fetch_all(&self.db)
        .await
    }

    /// Removes the passkey `id` from `session`'s Account, unless that would leave it no way to
    /// sign in.
    pub(crate) async fn remove_passkey(
        &self,
        session: &AccountSession,
        id: Uuid,
    ) -> Result<(), Error> {
        require_recent(session)?;
        let account = session.account.id;
        let mut tx = self.db.begin().await?;
        // The Account's row lock makes removals of its passkeys take turns. What's left is read in
        // statements of their own after it: under READ COMMITTED, the locking statement would see
        // them as they were before it waited.
        if !sessions::lock_while_recent(&mut tx, session).await? {
            return Err(Error::RecentAuthenticationRequired);
        }
        let has_password = sqlx::query_scalar!(
            r#"SELECT password_hash IS NOT NULL AS "has_password!" FROM accounts WHERE id = $1"#,
            account.0,
        )
        .fetch_one(&mut *tx)
        .await?;
        let removed = sqlx::query!(
            "DELETE FROM account_passkeys WHERE id = $1 AND account_id = $2",
            id,
            account.0,
        )
        .execute(&mut *tx)
        .await?;
        if removed.rows_affected() == 0 {
            return Err(Error::PasskeyNotFound);
        }
        let passkeys_left = sqlx::query_scalar!(
            r#"SELECT count(*) AS "left!" FROM account_passkeys WHERE account_id = $1"#,
            account.0,
        )
        .fetch_one(&mut *tx)
        .await?;
        if !can_sign_in(has_password, passkeys_left, self.sends_sign_in_links()) {
            return Err(Error::LastSignInMethod);
        }
        tx.commit().await?;
        Ok(())
    }

    fn challenge(&self, ceremony: Ceremony, caller: Caller) -> Result<Challenge, Error> {
        let relying_party = self.relying_party()?;
        self.challenges.admit(caller)?;
        Ok(Challenge {
            bytes: relying_party.challenge(ceremony, caller.now)?,
            rp_id: relying_party.id.clone(),
        })
    }

    fn relying_party(&self) -> Result<&RelyingParty, Error> {
        self.relying_party.as_deref().ok_or(Error::PasskeysOff)
    }
}

/// Fails unless `session` has Recent Authentication, checked again under the Account's row lock
/// before anything changes.
fn require_recent(session: &AccountSession) -> Result<(), Error> {
    if session.authenticated_recently {
        Ok(())
    } else {
        Err(Error::RecentAuthenticationRequired)
    }
}

/// Whether an Account with `password`, `passkeys` and, if the Hub sends them, Sign-in Links has a
/// way left to sign in.
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "each says whether the Account has one Sign-in Method; enums would only rename them"
)]
fn can_sign_in(password: bool, passkeys: i64, sign_in_links: bool) -> bool {
    password || passkeys > 0 || sign_in_links
}

#[cfg(test)]
#[path = "passkeys_tests.rs"]
mod tests;
