use tokio::net::TcpListener;

use super::{Probe as _, Tcp};

#[tokio::test]
async fn a_public_port_is_reached_only_while_something_listens_there() {
    let listening = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let open = listening.local_addr().unwrap();
    let closed = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();

    assert!(Tcp.reaches(open).await);
    assert!(!Tcp.reaches(closed).await);
}
