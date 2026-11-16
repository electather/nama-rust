//! Whose Requests and whose playing a User sees. The Requests area and the events filter both ask
//! it, so a User's event stream and Request list agree.

use nama_proto::nama::v1::{Permission, Requester};

use super::User;

impl User {
    /// Whose Requests this User sees: everyone's (`None`), or only those they are a Requester of.
    pub(crate) fn whose_requests(&self) -> Option<i64> {
        (!self.permissions.allow(Permission::ViewRequests)).then_some(self.id)
    }

    /// Whose playing this User sees: everyone's (`None`) with `ADMIN`, or else only their own.
    pub(crate) fn whose_playing(&self) -> Option<i64> {
        (!self.permissions.allow(Permission::Admin)).then_some(self.id)
    }
}

/// Whether `viewer`, as [`User::whose_requests`] gives one, may see something `requesters` asked
/// for: everyone's if `None`, else only what that User is among the Requesters of.
pub(crate) fn may_see(viewer: Option<i64>, requesters: &[Requester]) -> bool {
    viewer.is_none_or(|user| is_requester(user, requesters))
}

/// Whether `user` is among `requesters`.
pub(crate) fn is_requester(user: i64, requesters: &[Requester]) -> bool {
    requesters.iter().any(|requester| requester.user_id == user)
}

#[cfg(test)]
#[path = "seeing_tests.rs"]
mod tests;
