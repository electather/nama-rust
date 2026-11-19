use std::time::Duration;

use super::tests::Fixture;
use crate::{library::LibraryKind, plugins::ExtensionPoints};

#[tokio::test]
async fn shutting_down_waits_for_a_media_delete_already_started() {
    let media = Fixture::with_plugins(LibraryKind::Movies, ExtensionPoints::default()).await;
    media.write("Alien (1979)/Alien (1979).mkv", 10);
    media.scan().await;
    let alien = media
        .movie("Alien (1979)/Alien (1979).mkv", 348, "Alien")
        .await;
    // Holding the scan lock keeps the delete waiting before it deletes anything.
    let scan = media.scanner.lock(media.library).await;
    let deleting = media.scanner.clone();
    let delete =
        tokio::spawn(
            async move { (deleting.delete_media(alien, Vec::new(), "ada".to_owned())).await },
        );
    // Bounded, so a delete that never reaches the tracker fails the test below instead of hanging.
    for _ in 0..500 {
        if !media.scanner.inner.tasks.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let stopping = media.scanner.clone();
    let mut shutdown = tokio::spawn(async move { stopping.shutdown().await });
    let early = tokio::time::timeout(Duration::from_millis(200), &mut shutdown).await;
    assert!(
        early.is_err(),
        "shutdown returned while a delete was waiting"
    );
    assert!(media.path("Alien (1979)/Alien (1979).mkv").exists());

    drop(scan);
    shutdown.await.unwrap();
    assert!(
        !media.path("Alien (1979)/Alien (1979).mkv").exists(),
        "shutdown returned before the delete finished"
    );
    let deleted = delete.await.unwrap().unwrap();
    assert_eq!(deleted.files.len(), 1);
}
