//! The Server's VAPID key, which signs its Web Push (RFC 8292).

use crate::{db::Database, web_push::VapidKey};

/// The Server's VAPID key, made and stored the first time it's asked for, then the same for good,
/// as every browser subscribed with its public key.
///
/// # Errors
///
/// If reading or storing it fails, or the stored key isn't one.
pub(crate) async fn web_push_key(db: &Database) -> Result<VapidKey, sqlx::Error> {
    let stored = sqlx::query_scalar!("SELECT web_push_key FROM server_settings")
        .fetch_optional(db.reader())
        .await?
        .flatten();
    let pkcs8 = match stored {
        Some(pkcs8) => pkcs8,
        None => store_new_key(db).await?,
    };
    VapidKey::from_pkcs8(&pkcs8).map_err(|rejected| sqlx::Error::Decode(Box::new(rejected)))
}

/// Makes a key and stores it, returning the one stored: another caller's made at the same time
/// stays, so all agree.
async fn store_new_key(db: &Database) -> Result<Vec<u8>, sqlx::Error> {
    let made = VapidKey::new_pkcs8()
        .map_err(|err| sqlx::Error::Encode(format!("making a key: {err}").into()))?;
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query_scalar!(
                r#"INSERT INTO server_settings (id, url, web_push_key) VALUES (1, '', ?)
                   ON CONFLICT (id) DO UPDATE
                       SET web_push_key = coalesce(web_push_key, excluded.web_push_key)
                   RETURNING web_push_key AS "web_push_key!""#,
                made
            )
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
}
