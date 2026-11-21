use std::fs;

use notify::{
    EventKind,
    event::{CreateKind, DataChange, ModifyKind, RemoveKind},
};

use super::*;

#[test]
fn a_change_inside_an_ignored_folder_signals_no_scan_but_creating_or_removing_its_marker_does() {
    let media = tempfile::tempdir().unwrap();
    let root = media.path().to_owned();
    fs::create_dir_all(root.join("Samples/Deep")).unwrap();
    fs::write(root.join("Samples/.nomedia"), "").unwrap();
    let folders = Mutex::new(HashMap::from([(root.clone(), 7)]));
    let (sender, mut changes) = mpsc::unbounded_channel();
    let mut signalled = |kind: EventKind, relative: &str| {
        let event = Event::new(kind).add_path(root.join(relative));
        send_changed_libraries(&folders, &sender, Ok(event));
        changes.try_recv().ok()
    };

    let created = EventKind::Create(CreateKind::File);
    assert_eq!(signalled(created, "Samples/Deep/Up (2009).mkv"), None);
    assert_eq!(signalled(created, "Samples/Up (2009).mkv"), None);
    assert_eq!(signalled(created, "Samples/.nomedia"), Some(7));
    assert_eq!(signalled(created, "Alien (1979).mkv"), Some(7));

    fs::remove_file(root.join("Samples/.nomedia")).unwrap();
    assert_eq!(
        signalled(EventKind::Remove(RemoveKind::File), "Samples/.nomedia"),
        Some(7)
    );
}

#[test]
fn an_nfo_file_added_edited_or_deleted_signals_a_scan() {
    let media = tempfile::tempdir().unwrap();
    let root = media.path().to_owned();
    let folders = Mutex::new(HashMap::from([(root.clone(), 7)]));
    let (sender, mut changes) = mpsc::unbounded_channel();
    let nfo = root.join("Alien (1979)/Alien (1979).nfo");

    for kind in [
        EventKind::Create(CreateKind::File),
        EventKind::Modify(ModifyKind::Data(DataChange::Content)),
        EventKind::Remove(RemoveKind::File),
    ] {
        let event = Event::new(kind).add_path(nfo.clone());
        send_changed_libraries(&folders, &sender, Ok(event));
        assert_eq!(changes.try_recv().ok(), Some(7), "{kind:?}");
    }
}
