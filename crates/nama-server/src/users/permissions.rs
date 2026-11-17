//! The Permissions a User holds.

use buffa::Enumeration;
use nama_proto::nama::v1::Permission;

/// The set of Permissions a User holds (ADR 0020).
// Stored in `users.permissions` as a bitset: bit n is the `Permission` numbered n. Those numbers
// are API contract, so they never change under stored rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(transparent)]
pub(crate) struct Permissions(pub(super) i64);

impl Permissions {
    /// What the first Admin gets.
    pub(crate) const ADMIN: Self = Self(bit(Permission::Admin));

    /// What a new User gets until the default set is changed: what a new Server's starts as
    /// (`migrations/0002_default_permissions.sql`).
    #[cfg(test)]
    pub(crate) const NEW_USER: Self =
        Self(bit(Permission::Request) | bit(Permission::ViewRequests));

    /// Whether these pass a check for `needed`: they hold it, `PERMISSION_ADMIN`, or the
    /// Permission that covers it, as `PERMISSION_REQUEST` covers `PERMISSION_REQUEST_MOVIE` and
    /// `PERMISSION_REQUEST_TV`, and `PERMISSION_AUTO_APPROVE` its variants, as in Seerr.
    pub(crate) const fn allow(self, needed: Permission) -> bool {
        let covering = match needed {
            Permission::RequestMovie | Permission::RequestTv => Permission::Request,
            Permission::AutoApproveMovie | Permission::AutoApproveTv => Permission::AutoApprove,
            // Listed, not a wildcard, so a new Permission gets a decision about what covers it.
            Permission::Unspecified
            | Permission::Admin
            | Permission::ManageUsers
            | Permission::ManageLibraries
            | Permission::ManageSettings
            | Permission::ManageRequests
            | Permission::Request
            | Permission::AutoApprove
            | Permission::ViewRequests
            | Permission::ChooseQuality
            | Permission::DeleteMedia => needed,
        };
        self.0 & (bit(Permission::Admin) | bit(needed) | bit(covering)) != 0
    }

    /// Each Permission held, in the order of their values.
    pub(crate) fn iter(self) -> impl Iterator<Item = Permission> {
        Permission::values()
            .iter()
            .copied()
            .filter(move |&permission| self.0 & bit(permission) != 0)
    }
}

impl FromIterator<Permission> for Permissions {
    fn from_iter<I: IntoIterator<Item = Permission>>(iter: I) -> Self {
        Self(iter.into_iter().fold(0, |bits, p| bits | bit(p)))
    }
}

/// The bit `permission` is stored as in [`Permissions`].
// ponytail: an `i64` holds Permissions numbered up to 62; past that, `users.permissions` needs
// another encoding.
const fn bit(permission: Permission) -> i64 {
    1 << permission as i32
}
