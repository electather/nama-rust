//! Giving a User a Maximum Age through `UserService`, as an Admin's client does.

use connectrpc::{
    ConnectError,
    client::{ClientConfig, HttpClient},
};
use nama_proto::nama::v1::{MaximumAge, SetMaximumAgeRequest, UserServiceClient};

/// Has `by` give the User `user` the Maximum Age `max`, an age and whether unrated titles reach
/// them, or none if it's `None`.
pub(crate) async fn set_maximum_age(
    by: &ClientConfig,
    user: i64,
    max: Option<(u32, bool)>,
) -> Result<(), ConnectError> {
    let users = UserServiceClient::new(HttpClient::plaintext(), by.clone());
    let maximum_age = max.map(|(age, allow_unrated)| MaximumAge {
        age,
        allow_unrated,
        ..Default::default()
    });
    let request = SetMaximumAgeRequest {
        user_id: user,
        maximum_age: maximum_age.into(),
        ..Default::default()
    };
    users.set_maximum_age(request).await.map(drop)
}
