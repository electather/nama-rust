use std::sync::Arc;

use crate::{
    BoxFuture,
    automation::{Kernel, KernelRefused, answering, functions::ListUsers},
    contract::{User, Users, UsersAsked},
};

fn ada() -> Users {
    Users {
        users: vec![User {
            id: 1,
            display_name: "Ada".to_owned(),
            ..User::default()
        }],
        ..Users::default()
    }
}

#[tokio::test]
async fn ask_answers_the_functions_response() {
    let kernel = answering::<ListUsers>(|_asked| Ok(ada()));

    let listed = kernel.ask::<ListUsers>(UsersAsked::default()).await;

    assert_eq!(listed, Ok(ada()));
}

#[tokio::test]
async fn an_answer_ask_cant_read_is_a_refusal() {
    let unreadable = |_function: &str, _request: Vec<u8>| -> BoxFuture<'static, _> {
        Box::pin(async { Ok(vec![0xff]) })
    };
    let kernel: Arc<dyn Kernel> = Arc::new(unreadable);

    let refused = kernel.ask::<ListUsers>(UsersAsked::default()).await;

    let words = "the Kernel's answer to list_users can't be read".to_owned();
    assert_eq!(refused, Err(KernelRefused(words)));
}

#[tokio::test]
async fn answering_refuses_any_other_function() {
    let kernel = answering::<ListUsers>(|_asked| Ok(ada()));

    let refused = kernel.call("list_watchlist", Vec::new()).await;

    let words = "the Kernel has no function list_watchlist".to_owned();
    assert_eq!(refused, Err(KernelRefused(words)));
}
