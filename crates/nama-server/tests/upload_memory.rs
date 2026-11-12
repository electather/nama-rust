//! An upload is written to disk as it arrives, never held whole in memory. Alone in its binary, so
//! the allocator it counts with sees no other test's work.

#[path = "support/browsing.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod browsing;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
};

use browsing::Fixture;
use futures_util::stream;
use http::send;
use http_body_util::StreamBody;
use hyper::{Request, body::Bytes, body::Frame};
use image::{ImageFormat, RgbImage};

/// The system allocator, counting the bytes allocated now and the most there have been.
struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

#[expect(
    unsafe_code,
    reason = "counting what the Server allocates needs a global allocator, which is unsafe to \
              implement; it only forwards to `System`"
)]
// SAFETY: each method hands the call to `System` unchanged and only counts the bytes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(now, Ordering::Relaxed);
        // SAFETY: the caller upholds `alloc`'s contract, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller upholds `dealloc`'s contract, which `System` shares.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

const MIB: usize = 1024 * 1024;

#[tokio::test]
async fn a_20_mib_upload_does_not_raise_memory_by_20_mib() {
    let fx = Fixture::alien().await;
    let mut png = Cursor::new(Vec::new());
    RgbImage::new(2, 3)
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    // A PNG is read only as far as its header, so what follows its end needn't be pixels.
    let mut start = png.into_inner();
    start.resize(MIB, 0);
    let chunks = (0..20).map(move |i| {
        let chunk = if i == 0 { start.clone() } else { vec![0; MIB] };
        Ok::<_, std::convert::Infallible>(Frame::data(Bytes::from(chunk)))
    });
    let request = Request::put("/custom-images/movie/tmdb-348/poster")
        .body(StreamBody::new(stream::iter(chunks)))
        .unwrap();
    let before = ALLOCATED.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);

    let reply = send(&fx.server, request, Some(&fx.server.admin_token)).await;

    assert_eq!(reply.status, 204);
    let raised = PEAK.load(Ordering::Relaxed) - before;
    assert!(raised < 10 * MIB, "memory rose by {} MiB", raised / MIB);
}
