//! A User's Account as a way in: GLOSSARY counts an Account among the Sign-in Methods, but the
//! Kernel stores only the others (ADR 0014), so a way in is a stored Sign-in Method or a Link the
//! Plugin that links the Server says is Linked. Creating a User from an Account (ADR 0092) too.

use futures_util::future::join_all;

use super::{Error, User, Users, checked_name};
use crate::link::{self, Link};

/// How a User can sign in.
enum WayIn {
    /// With a Sign-in Method the Kernel stores.
    SignInMethod,
    /// Only through their Link to an Account.
    Account,
    /// Not at all.
    Nothing,
    /// Not with a stored Sign-in Method, and the Plugin couldn't tell whether they are Linked.
    Unknown,
}

impl Users {
    /// Which Users can't sign in: with no stored Sign-in Method and no Link. One whose Link the
    /// Plugin couldn't tell is left out, as they may be Linked.
    // ponytail: one call per User with no Sign-in Method, all at once, though the Plugin asks the
    // Hub about each at most once a minute; cap them if Servers come to have many such Users.
    pub(crate) async fn cannot_sign_in(&self) -> Result<Vec<i64>, sqlx::Error> {
        let asked = (self.without_sign_in_method().await?.into_iter())
            .map(async |user_id| (user_id, self.account_way_in(user_id).await));
        let ways_in = join_all(asked).await.into_iter();
        Ok(ways_in
            .filter_map(|(user_id, way_in)| matches!(way_in, WayIn::Nothing).then_some(user_id))
            .collect())
    }

    /// Unlinks the User `user_id` from their Account, or stops showing their code, unless their
    /// Account is their only way in ([`Error::LastSignInMethod`]), which ADR 0014 says a User
    /// always keeps, or the Plugin couldn't tell it isn't ([`Error::LinkingFailed`]). The
    /// service may still Unlink them (ADR 0099), and an Admin's `ResetPassword` gets them back in.
    pub(crate) async fn unlink(&self, user_id: i64) -> Result<(), Error> {
        match self.way_in(user_id).await? {
            WayIn::Account => return Err(Error::LastSignInMethod),
            WayIn::Unknown => return Err(Error::LinkingFailed),
            WayIn::SignInMethod | WayIn::Nothing => {}
        }
        // ponytail: a code the Hub approves between the check and this Unlink is cancelled, and so
        // Unlinked, unchecked; the window is one Hub round trip.
        Ok(self.linking.unlink_user(user_id).await?)
    }

    /// Creates a User named `name` with the default Permissions and no Sign-in Method, as `by`
    /// asks, and has the Account with `email` asked to accept them, which Links them to it: they
    /// sign in once it does (ADR 0092). Refused as [`Users::create`] refuses. A User the Plugin
    /// couldn't be asked about is deleted again.
    pub(crate) async fn create_from_account(
        &self,
        by: &User,
        name: &str,
        email: &str,
    ) -> Result<i64, Error> {
        let name = checked_name("name", "a User's name", name)?;
        let (users, by, email) = (self.clone(), by.clone(), email.to_owned());
        // Spawned, so a client that leaves mid-way can't drop it between creating the User and
        // deleting them again, which would leave a User with no way to sign in.
        tokio::spawn(async move {
            let id = users.insert_created(&by, name.clone(), None).await?;
            let invited = users.linking.invite_user(id, &name, &email).await;
            if invited.is_err() {
                users.delete_uninvited(&by, id).await;
            }
            Ok(invited.map(|()| id)?)
        })
        .await?
    }

    /// Deletes the User `id`, just created from an Account the Hub couldn't be told of, as `by`
    /// asks, logging a failure.
    async fn delete_uninvited(&self, by: &User, id: i64) {
        if let Err(not_deleted) = self.delete(by, id).await {
            tracing::error!(
                user = id,
                "could not delete a User the Hub wasn't told of: {not_deleted}"
            );
        }
    }

    /// The User `user_id`'s way in.
    async fn way_in(&self, user_id: i64) -> Result<WayIn, sqlx::Error> {
        if self.has_sign_in_method(user_id).await? {
            return Ok(WayIn::SignInMethod);
        }
        Ok(self.account_way_in(user_id).await)
    }

    /// The way in of the User `user_id`, who has no stored Sign-in Method.
    async fn account_way_in(&self, user_id: i64) -> WayIn {
        match self.linking.user_link(user_id).await {
            Ok(Some(Link::Linked { .. })) => WayIn::Account,
            Ok(_) | Err(link::Error::NoLinker) => WayIn::Nothing,
            Err(link::Error::Failed) => WayIn::Unknown,
        }
    }

    /// The ids of the Users with no Sign-in Method, oldest first: each signs in only through their
    /// Link to an Account, if they have one (ADR 0092).
    async fn without_sign_in_method(&self) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT id AS "id!" FROM users u
               WHERE NOT EXISTS (SELECT 1 FROM sign_in_methods m WHERE m.user_id = u.id)
               ORDER BY id"#
        )
        .fetch_all(self.db.reader())
        .await
    }

    /// Whether the User `user` has a Sign-in Method the Kernel stores.
    pub(super) async fn has_sign_in_method(&self, user: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM sign_in_methods WHERE user_id = ?) AS "exists!: bool""#,
            user
        )
        .fetch_one(self.db.reader())
        .await
    }
}

impl From<link::Error> for Error {
    fn from(err: link::Error) -> Self {
        match err {
            link::Error::NoLinker => Self::NoLinkingPlugin,
            link::Error::Failed => Self::LinkingFailed,
        }
    }
}
