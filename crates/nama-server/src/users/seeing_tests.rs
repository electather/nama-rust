use nama_proto::nama::v1::{Permission, Requester};

use super::*;
use crate::users::Permissions;

fn user(id: i64, permissions: Permissions) -> User {
    User {
        id,
        name: "Ada".to_owned(),
        permissions,
        created_at: 0,
    }
}

fn requesters(ids: &[i64]) -> Vec<Requester> {
    (ids.iter())
        .map(|&user_id| Requester {
            user_id,
            ..Default::default()
        })
        .collect()
}

#[test]
fn a_user_who_may_view_requests_sees_everyones() {
    let user = user(7, std::iter::once(Permission::ViewRequests).collect());
    assert_eq!(user.whose_requests(), None);
}

#[test]
fn a_user_who_may_not_view_requests_sees_only_their_own() {
    let user = user(7, std::iter::once(Permission::Request).collect());
    assert_eq!(user.whose_requests(), Some(7));
}

#[test]
fn only_an_admin_sees_everyones_playing() {
    assert_eq!(user(7, Permissions::ADMIN).whose_playing(), None);
    assert_eq!(user(7, Permissions::NEW_USER).whose_playing(), Some(7));
}

#[test]
fn a_viewer_of_their_own_requests_sees_only_what_they_are_a_requester_of() {
    assert!(may_see(None, &requesters(&[1])));
    assert!(may_see(Some(1), &requesters(&[2, 1])));
    assert!(!may_see(Some(3), &requesters(&[2, 1])));
}
