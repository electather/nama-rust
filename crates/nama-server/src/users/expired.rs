//! Deleting expired Devices and Invites. Neither works once expired, but each stays stored until
//! deleted, which Upkeep's Job asks for (ADR 0079).

use super::{Users, invites::delete_expired_invites};
use crate::db::now;

/// How many expired Devices and Invites [`Users::delete_expired`] deleted.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ExpiredDeleted {
    pub(crate) devices: usize,
    pub(crate) invites: u64,
}

impl Users {
    /// Deletes every expired Device, signing it out as [`Users::sign_out`] does, and every expired
    /// Invite.
    pub(crate) async fn delete_expired(&self) -> Result<ExpiredDeleted, sqlx::Error> {
        let now = now();
        let invites = (self.db)
            .write(move |tx| Box::pin(async move { delete_expired_invites(tx, now).await }))
            .await?;
        let devices = self
            .write_signing_out(move |tx| {
                Box::pin(async move {
                    sqlx::query_scalar!(
                        r#"DELETE FROM devices WHERE expires_at <= ? RETURNING id AS "id!""#,
                        now
                    )
                    .fetch_all(&mut **tx)
                    .await
                })
            })
            .await?;
        Ok(ExpiredDeleted { devices, invites })
    }
}

#[cfg(test)]
#[path = "expired_tests.rs"]
mod tests;
