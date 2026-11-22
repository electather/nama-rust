//! Deleting a User tells the Plugin that links the Server to Unlink them, without waiting for it
//! (ADR 0011).

#[path = "support/log.rs"]
mod log;
#[path = "support/scripted_linker.rs"]
mod scripted_linker;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use log::Log;
use nama_plugin_api::server_link::Link;
use nama_proto::nama::v1::{ListUsersRequest, Permission};
use scripted_linker::{Scripted, delete, until_unlinked, users, with_linker};
use server::TestServer;

#[tokio::test]
async fn deleting_a_user_unlinks_them_once() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::Linked {
        account: "grace@example.com".to_owned(),
        reachability: None,
    });
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();

    until_unlinked(&linker, &[grace.id]).await;
}

#[tokio::test]
async fn deleting_a_user_doesnt_wait_for_the_plugin_to_unlink_them() {
    let linker = Scripted::default();
    linker.script().unlink_user_hangs = true;
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    let deleted =
        tokio::time::timeout(server::WAIT, delete(server.client_config(), grace.id)).await;

    deleted.expect("the delete waited for the Plugin").unwrap();
    until_unlinked(&linker, &[grace.id]).await;
}

#[tokio::test]
async fn an_admin_deleting_themselves_is_unlinked() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;
    let ada = server.user("Ada", &[Permission::Admin]).await;

    delete(ada.client_config(), ada.id).await.unwrap();

    until_unlinked(&linker, &[ada.id]).await;
}

#[tokio::test]
async fn an_unlinked_user_is_deleted() {
    let linker = Scripted::default();
    linker.script().user_link = Some(Link::NotLinked);
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();
}

#[tokio::test]
async fn a_refused_delete_unlinks_no_one() {
    let linker = Scripted::default();
    let server = with_linker(&linker).await;

    delete(server.client_config(), 9999).await.unwrap_err();

    assert_eq!(linker.script().unlinked_users, Vec::<i64>::new());
}

#[tokio::test]
async fn a_user_is_deleted_without_a_plugin_that_links_the_server() {
    let server = TestServer::builder().start().await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();
}

#[tokio::test]
async fn a_plugin_that_cant_unlink_a_deleted_user_leaves_them_deleted_and_warns() {
    let log = Log::global();
    let linker = Scripted::default();
    linker.script().unlink_user_fails = true;
    let server = with_linker(&linker).await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();

    let listed = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users;
    assert!(listed.iter().all(|user| user.id != grace.id));
    log.wait_for(
        &["could not tell the service a User was deleted", "linker"],
        1,
    )
    .await;
}
