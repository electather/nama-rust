use nama_plugin_api::auth::PasswordError;

use crate::plugins::{FailureKind as _, ProviderError as _};

#[test]
fn a_rejected_password_is_no_failure_while_a_failed_hash_is_one() {
    let rejected = PasswordError::Rejected("too short".to_owned()).failure();
    let failed = PasswordError::Failed("out of memory".into()).failure();

    assert_eq!(
        (rejected.kind.is_failure(), failed.kind.is_failure()),
        (false, true)
    );
}
