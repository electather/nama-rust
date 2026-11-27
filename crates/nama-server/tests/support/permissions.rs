//! Giving a User Permissions through `UserService`, as an Admin's client does.

use connectrpc::{
    ConnectError,
    client::{ClientConfig, HttpClient},
};
use nama_proto::nama::v1::{Permission, SetPermissionsRequest, UserServiceClient};

/// Has `by` give the User `user` exactly `permissions`.
pub(crate) async fn set_permissions(
    by: &ClientConfig,
    user: i64,
    permissions: &[Permission],
) -> Result<(), ConnectError> {
    let users = UserServiceClient::new(HttpClient::plaintext(), by.clone());
    let request = SetPermissionsRequest {
        user_id: user,
        permissions: permissions.iter().map(|&p| p.into()).collect(),
        ..Default::default()
    };
    users.set_permissions(request).await.map(drop)
}
