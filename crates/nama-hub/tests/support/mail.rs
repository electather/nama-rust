//! A stand-in for the mail service, which keeps what the Hub sends for the test to read.

use std::{future, sync::Arc, time::Duration};

use nama_hub::mail::{Email, Mail, Mailer, Sending};
use tokio::{sync::mpsc, time::timeout};

/// The address the fake's links lead to, with the trailing slash an operator may type.
pub(crate) const PUBLIC_URL: &str = "https://hub.example.com/";

/// What the Hub sent through the fake, oldest first.
#[derive(Debug)]
pub(crate) struct Inbox(mpsc::UnboundedReceiver<Email>);

#[derive(Debug)]
struct FakeMailer(mpsc::UnboundedSender<Email>);

impl Mailer for FakeMailer {
    fn send(&self, email: Email) -> Sending<'_> {
        // The test may have stopped reading; its assertions are what matter then.
        let _unread = self.0.send(email);
        Box::pin(future::ready(Ok(())))
    }
}

/// Mail through the fake, and the inbox that receives it.
pub(crate) fn fake_mail() -> (Mail, Inbox) {
    let (sent, received) = mpsc::unbounded_channel();
    let mail = Mail {
        mailer: Arc::new(FakeMailer(sent)),
        public_url: PUBLIC_URL.to_owned(),
    };
    (mail, Inbox(received))
}

#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
impl Inbox {
    /// The next email the Hub sends, which the Hub sends off the request path, so it waits a
    /// while for one.
    pub(crate) async fn next(&mut self) -> Email {
        timeout(Duration::from_secs(10), self.0.recv())
            .await
            .expect("the Hub should have sent an email")
            .expect("the Hub keeps its mail while serving")
    }

    /// Whether the Hub has sent nothing that wasn't read yet.
    pub(crate) fn is_empty(&mut self) -> bool {
        self.0.try_recv().is_err()
    }

    /// The token of the next Sign-in Link the Hub sends, after checking it goes to `to`.
    pub(crate) async fn next_link(&mut self, to: &str) -> String {
        let email = self.next().await;
        assert_eq!(email.to, to);
        let prefix = "https://hub.example.com/sign-in-link#";
        let link = email
            .text
            .lines()
            .find_map(|line| line.strip_prefix(prefix))
            .expect("the email holds a Sign-in Link");
        link.to_owned()
    }
}
