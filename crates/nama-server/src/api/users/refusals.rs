//! What a client gets when a change to the Users fails.

use buffa::Enumeration as _;
use connectrpc::ConnectError;
use nama_proto::nama::v1::{Permission, RefusalReason};

use crate::{
    api::{access::permission_required, database_failed, refuse, server_link},
    users,
};

/// The Connect error a client gets for a change to the Users that failed. Server-side failures
/// are logged, not sent.
impl From<users::Error> for ConnectError {
    fn from(err: users::Error) -> Self {
        use RefusalReason as R;
        let message = err.to_string();
        let refused = match err {
            users::Error::NameLength { field, .. } => {
                refuse(R::REFUSAL_REASON_NAME_LENGTH, message)
                    .field(field)
                    .param("minLength", users::NAME_LENGTH.start())
                    .param("maxLength", users::NAME_LENGTH.end())
            }
            users::Error::NameCharacter {
                field, character, ..
            } => refuse(R::REFUSAL_REASON_NAME_CHARACTER, message)
                .field(field)
                .param("character", &format!("U+{:04X}", u32::from(character))),
            users::Error::PasswordRejected { field, .. } => {
                refuse(R::REFUSAL_REASON_PASSWORD_REJECTED, message).field(field)
            }
            users::Error::SwitchPinInvalid => refuse(R::REFUSAL_REASON_SWITCH_PIN_INVALID, message)
                .field("switch_pin")
                .param("minLength", users::SWITCH_PIN_LENGTH.start())
                .param("maxLength", users::SWITCH_PIN_LENGTH.end()),
            // Only creating a User or setting up the first one takes a name to check.
            users::Error::NameTaken => {
                refuse(R::REFUSAL_REASON_USER_NAME_TAKEN, message).field("name")
            }
            // Every call that names a User names them in `user_id`.
            users::Error::NoSuchUser => {
                refuse(R::REFUSAL_REASON_USER_NOT_FOUND, message).field("user_id")
            }
            users::Error::NoSuchDevice => {
                refuse(R::REFUSAL_REASON_DEVICE_NOT_FOUND, message).field("device_id")
            }
            users::Error::InviteNotFound { field } => {
                refuse(R::REFUSAL_REASON_INVITE_NOT_FOUND, message).field(field)
            }
            // Only making an Invite takes Permissions to grant.
            users::Error::NotGrantable(permission) => {
                refuse(R::REFUSAL_REASON_PERMISSION_REQUIRED, message)
                    .field("permissions")
                    .param("permission", &permission.proto_name())
            }
            // A polled Sign-in Code names no User.
            users::Error::ApprovingUserDeleted => refuse(R::REFUSAL_REASON_USER_NOT_FOUND, message),
            users::Error::AlreadySetUp => refuse(R::REFUSAL_REASON_ALREADY_SET_UP, message),
            users::Error::LastAdmin => refuse(R::REFUSAL_REASON_LAST_ADMIN, message),
            users::Error::LastSignInMethod => {
                refuse(R::REFUSAL_REASON_LAST_SIGN_IN_METHOD, message)
            }
            users::Error::NoHub => return server_link::no_hub(),
            users::Error::LinkingFailed => {
                return Self::unavailable(server_link::LINKING_FAILED);
            }
            users::Error::AdminRequired => return permission_required(Permission::Admin),
            users::Error::WrongNameOrPassword => {
                refuse(R::REFUSAL_REASON_WRONG_NAME_OR_PASSWORD, message)
            }
            users::Error::WrongPassword => {
                refuse(R::REFUSAL_REASON_WRONG_PASSWORD, message).field("password")
            }
            users::Error::SignedOut | users::Error::ApprovingDeviceSignedOut => {
                refuse(R::REFUSAL_REASON_SIGNED_OUT, message)
            }
            users::Error::Database(err) => return database_failed(err),
            // Only creating a User from an Account runs in a task of its own.
            users::Error::Panicked(_) => {
                tracing::error!("{err}");
                return Self::internal("the Server failed to create the User");
            }
            users::Error::NoAuthenticator
            | users::Error::Authenticator(_)
            | users::Error::SwitchPinHashing(_)
            | users::Error::Random(_) => {
                tracing::error!("a change to the Users failed: {err}");
                return Self::internal("the Server failed");
            }
        };
        refused.into()
    }
}
