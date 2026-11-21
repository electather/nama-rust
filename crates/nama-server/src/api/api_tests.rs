use connectrpc::ErrorCode;

use super::database_failed;

#[test]
fn a_pool_timeout_is_unavailable_and_another_database_failure_internal() {
    assert_eq!(
        database_failed(sqlx::Error::PoolTimedOut).code,
        ErrorCode::Unavailable
    );
    assert_eq!(
        database_failed(sqlx::Error::RowNotFound).code,
        ErrorCode::Internal
    );
}
