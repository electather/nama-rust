//! Where a [`TestServer`] keeps the artwork Matches store, for tests of what it keeps and drops.

use std::{path::PathBuf, time::Duration};

use crate::server::{TestServer, WAIT};

/// Where `server` keeps the image at `path` from the Plugin `plugin`'s title source, as the store
/// escapes its leading `/`.
pub(crate) fn kept(server: &TestServer, plugin: &str, path: &str) -> PathBuf {
    server
        .data_dir()
        .join("artwork")
        .join(plugin)
        .join(path.replacen('/', "%2F", 1))
}

/// Waits until `server` keeps each of `plugin`'s images at `paths`, failing the test after
/// [`WAIT`].
pub(crate) async fn until_kept(server: &TestServer, plugin: &str, paths: &[&str]) {
    let all_kept = || paths.iter().all(|path| kept(server, plugin, path).exists());
    let waited = tokio::time::timeout(WAIT, async {
        while !all_kept() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "never kept {paths:?}");
}
