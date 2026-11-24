//! Deleting a User tells the Hub to Unlink them, without waiting for it (ADR 0011, ADR 0105).

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_hub.rs"]
mod fake_hub;
#[path = "support/log.rs"]
mod log;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::{
    ConnectError,
    client::{ClientConfig, HttpClient},
};
use fake_hub::FakeHub;
use log::Log;
use nama_proto::nama::v1::{DeleteUserRequest, ListUsersRequest, Permission, UserServiceClient};
use server::TestServer;

/// A Server Linked to the fake Hub, and the Hub.
async fn linked_server() -> (TestServer, FakeHub) {
    let hub = FakeHub::start().await;
    let server = TestServer::builder().config(hub.config()).start().await;
    hub.link(&server).await;
    (server, hub)
}

fn users(config: ClientConfig) -> UserServiceClient<HttpClient> {
    UserServiceClient::new(HttpClient::plaintext(), config)
}

/// Deletes the User `user_id`, calling as `who`.
async fn delete(who: ClientConfig, user_id: i64) -> Result<(), ConnectError> {
    users(who)
        .delete_user(DeleteUserRequest {
            user_id,
            ..Default::default()
        })
        .await
        .map(drop)
}

#[tokio::test]
async fn deleting_a_linked_user_unlinks_them_once() {
    let (server, hub) = linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await;
    hub.link_user(grace.id);

    delete(server.client_config(), grace.id).await.unwrap();

    hub.until_unlinked(&[grace.id]).await;
}

#[tokio::test]
async fn deleting_a_user_doesnt_wait_for_the_hub_to_unlink_them() {
    let (server, hub) = linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await;
    hub.hang();

    let deleted =
        tokio::time::timeout(server::WAIT, delete(server.client_config(), grace.id)).await;

    deleted.expect("the delete waited for the Hub").unwrap();
}

#[tokio::test]
async fn an_admin_deleting_themselves_is_unlinked() {
    let (server, hub) = linked_server().await;
    let ada = server.user("Ada", &[Permission::Admin]).await;

    delete(ada.client_config(), ada.id).await.unwrap();

    hub.until_unlinked(&[ada.id]).await;
}

#[tokio::test]
async fn a_refused_delete_unlinks_no_one() {
    let (server, hub) = linked_server().await;

    delete(server.client_config(), 9999).await.unwrap_err();

    assert_eq!(hub.unlinked_users(), Vec::<i64>::new());
}

#[tokio::test]
async fn a_user_is_deleted_on_a_server_whose_config_names_no_hub() {
    let server = TestServer::builder().start().await;
    let grace = server.user_with_default_permissions("Grace").await;

    delete(server.client_config(), grace.id).await.unwrap();
}

#[tokio::test]
async fn a_hub_that_cant_unlink_a_deleted_user_leaves_them_deleted_and_warns() {
    let log = Log::global();
    let (server, hub) = linked_server().await;
    let grace = server.user_with_default_permissions("Grace").await;
    hub.fail_unlinks();

    delete(server.client_config(), grace.id).await.unwrap();

    let listed = users(server.client_config())
        .list_users(ListUsersRequest::default())
        .await
        .unwrap()
        .into_owned()
        .users;
    assert!(listed.iter().all(|user| user.id != grace.id));
    log.wait_for(&["could not tell the Hub a User was deleted"], 1)
        .await;
}
