use std::{
    pin::pin,
    task::{Context, Poll, Waker},
};

use super::{MetadataError, TitleSource};

struct NamespaceOnly;

impl TitleSource for NamespaceOnly {
    fn namespace(&self) -> &'static str {
        "tvdb"
    }
}

#[test]
fn a_method_a_source_leaves_out_is_unsupported() {
    let source = NamespaceOnly;
    let mut image = pin!(source.image("/poster.jpg"));

    let answer = image.as_mut().poll(&mut Context::from_waker(Waker::noop()));

    assert!(
        matches!(answer, Poll::Ready(Err(MetadataError::Unsupported))),
        "{answer:?}"
    );
}
