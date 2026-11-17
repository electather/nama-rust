use std::{ffi::OsStr, path::Path};

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

#[test]
fn an_image_named_alone_stands_in_only_outside_the_librarys_folders_and_one_named_after_the_file_anywhere()
 {
    let video = OsStr::new("Up (2009)");
    let role =
        |name: &str, in_library_folder| image_role(Path::new(name), video, in_library_folder);

    assert_eq!(
        role("poster.jpg", false),
        Some((Naming::Alone, Role::Poster))
    );
    assert_eq!(
        role("Folder.PNG", false),
        Some((Naming::Alone, Role::Poster))
    );
    assert_eq!(
        role("fanart.webp", false),
        Some((Naming::Alone, Role::Backdrop))
    );
    assert_eq!(role("poster.jpg", true), None);
    assert_eq!(
        role("Up (2009)-poster.jpeg", true),
        Some((Naming::AfterVideo, Role::Poster))
    );
    assert_eq!(
        role("up (2009)-Fanart.jpg", false),
        Some((Naming::AfterVideo, Role::Backdrop))
    );
    assert_eq!(role("Other-poster.jpg", false), None);
    assert_eq!(role("poster.gif", false), None);
    assert_eq!(role("banner.jpg", false), None);
}
