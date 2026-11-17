use std::path::Path;

use super::*;

#[test]
fn a_sidecar_belongs_to_the_video_with_the_longest_stem_its_name_starts_with() {
    let videos = [Path::new("/m/Up.mkv"), Path::new("/m/Up-1080p.mkv")];
    let video_of = |sidecar: &str| video_of(Path::new(sidecar), &videos);

    assert_eq!(video_of("/m/Up.en.srt"), Some(Path::new("/m/Up.mkv")));
    assert_eq!(
        video_of("/m/Up-1080p.en.srt"),
        Some(Path::new("/m/Up-1080p.mkv"))
    );
    assert_eq!(video_of("/m/Other.en.srt"), None);
}
