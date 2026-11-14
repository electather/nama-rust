use std::{
    pin::pin,
    task::{Context, Poll, Waker},
};

use super::{MetadataError, TitleSource};
use crate::contract::{ArtworkAsked, ImageAsked};

struct NamespaceOnly;

impl TitleSource for NamespaceOnly {
    fn namespace(&self) -> &'static str {
        "tvdb"
    }
}

#[test]
fn a_method_a_source_leaves_out_is_unsupported() {
    let source = NamespaceOnly;
    let asked = ImageAsked {
        path: "/poster.jpg".to_owned(),
        ..Default::default()
    };
    let mut image = pin!(source.image(&asked));
    let artwork_asked = ArtworkAsked::default();
    let mut artwork = pin!(source.artwork(&artwork_asked));
    let mut context = Context::from_waker(Waker::noop());

    let image = image.as_mut().poll(&mut context);
    let artwork = artwork.as_mut().poll(&mut context);

    assert!(
        matches!(image, Poll::Ready(Err(MetadataError::Unsupported))),
        "{image:?}"
    );
    assert!(
        matches!(artwork, Poll::Ready(Err(MetadataError::Unsupported))),
        "{artwork:?}"
    );
}
