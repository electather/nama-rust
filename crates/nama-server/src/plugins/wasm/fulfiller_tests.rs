use std::time::Duration;

use nama_plugin_api::fulfiller::TryAgainIn;

use super::try_again;

#[test]
fn try_again_clamps_to_one_through_three_hundred_seconds() {
    let secs = |seconds| Some(TryAgainIn(Duration::from_secs(seconds)));
    assert_eq!(try_again(None), None);
    assert_eq!(try_again(Some(0)), secs(1));
    assert_eq!(try_again(Some(1000)), secs(300));
    assert_eq!(try_again(Some(5)), secs(5));
}
