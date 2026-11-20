use std::fs;

use notify::{EventKind, event::CreateKind};

use super::*;

#[test]
fn a_change_inside_an_ignored_folder_signals_no_scan_but_its_marker_does() {
    let media = tempfile::tempdir().unwrap();
    let root = media.path().to_owned();
    fs::create_dir_all(root.join("Samples/Deep")).unwrap();
    fs::write(root.join("Samples/.nomedia"), "").unwrap();
    let folders = Mutex::new(HashMap::from([(root.clone(), 7)]));
    let (sender, mut changes) = mpsc::unbounded_channel();
    let mut signalled = |relative: &str| {
        let event = Event::new(EventKind::Create(CreateKind::File)).add_path(root.join(relative));
        send_changed_libraries(&folders, &sender, Ok(event));
        changes.try_recv().ok()
    };

    assert_eq!(signalled("Samples/Deep/Up (2009).mkv"), None);
    assert_eq!(signalled("Samples/Up (2009).mkv"), None);
    assert_eq!(signalled("Samples/.nomedia"), Some(7));
    assert_eq!(signalled("Alien (1979).mkv"), Some(7));
}
