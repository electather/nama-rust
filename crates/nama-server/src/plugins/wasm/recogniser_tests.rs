use std::sync::Arc;

use nama_plugin_api::{
    Registrar, Registration,
    contract::{
        FileToRecognise, LibraryKind, TitleId, TitleKind, TitleToRecognise,
        file_recognised::Recognised,
    },
    recogniser::{Recogniser, RecogniserMethod},
};
use nama_plugin_proto::{
    nama::plugin::v1::{Manifest, Point},
    services,
};

use super::super::{
    Running, check_exports, compile, load::write_cache_config, points, services::POINTS,
};

/// A Recogniser in WASM's text format, speaking Extism's guest ABI by hand: `recognise_title`
/// answers the ids `tvdb` `81189`, and `recognise_file` answers its request wrapped as field 1,
/// so a `FileToRecognise` comes back as a Movie titled by its path. Requests here are under 128
/// bytes, so one byte holds the wrapped length.
const RECOGNISER: &str = r#"
(module
  (import "extism:host/env" "input_length" (func $input_length (result i64)))
  (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
  (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
  (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))
  (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
  (memory 1)
  ;; TitleRecognised { ids: [TitleId { namespace: "tvdb", value: "81189" }] }, 15 bytes.
  (data (i32.const 0) "\0a\0d\0a\04tvdb\12\0581189")
  (func $recognise_title (result i32)
    (local $out i64) (local $i i32)
    (local.set $out (call $alloc (i64.const 15)))
    (block $done
      (loop $copy
        (br_if $done (i32.ge_u (local.get $i) (i32.const 15)))
        (call $store_u8
          (i64.add (local.get $out) (i64.extend_i32_u (local.get $i)))
          (i32.load8_u (local.get $i)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $copy)))
    (call $output_set (local.get $out) (i64.const 15))
    (i32.const 0))
  (func $recognise_file (result i32)
    (local $length i64) (local $out i64) (local $i i64)
    (local.set $length (call $input_length))
    (local.set $out (call $alloc (i64.add (local.get $length) (i64.const 2))))
    (call $store_u8 (local.get $out) (i32.const 0x0a))
    (call $store_u8 (i64.add (local.get $out) (i64.const 1)) (i32.wrap_i64 (local.get $length)))
    (block $done
      (loop $copy
        (br_if $done (i64.ge_u (local.get $i) (local.get $length)))
        (call $store_u8
          (i64.add (local.get $out) (i64.add (local.get $i) (i64.const 2)))
          (call $input_load_u8 (local.get $i)))
        (local.set $i (i64.add (local.get $i) (i64.const 1)))
        (br $copy)))
    (call $output_set (local.get $out) (i64.add (local.get $length) (i64.const 2)))
    (i32.const 0))
  (export "recognise_title" (func $recognise_title))
  (export "recognise_file" (func $recognise_file)))
"#;

/// [`RECOGNISER`] as the Kernel would hold it, registered through its `POINTS` row, with only
/// `exports` among its exports known to it.
fn recogniser(exports: &[&'static str]) -> Arc<dyn Recogniser> {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let running = Running {
        package: Arc::default(),
        compiled: Arc::new(
            compile(RECOGNISER.as_bytes(), &cache_config, Vec::new(), Vec::new()).unwrap(),
        ),
        secrets: Vec::new().into(),
        crashes: Arc::default(),
        exports: Arc::new(exports.iter().copied().collect()),
    };
    let point = POINTS
        .iter()
        .find(|point| point.service == services::RECOGNISER)
        .unwrap();
    let mut registrar = Registrar::new();
    (point.register)(&mut registrar, running);
    registrar
        .into_registrations()
        .iter()
        .find_map(Registration::provider::<dyn Recogniser>)
        .unwrap()
}

fn tvdb(value: &str) -> TitleId {
    TitleId {
        namespace: "tvdb".to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

#[test]
fn a_manifest_naming_the_recogniser_service_loads_with_either_export() {
    let package = Manifest {
        points: vec![Point {
            service: services::RECOGNISER.to_owned(),
            version: "^1.0.0".to_owned(),
            ..Point::default()
        }],
        ..Manifest::default()
    };
    let named = points(&package).unwrap();

    for export in ["recognise_title", "recognise_file"] {
        let checked = check_exports(&named, |has| has == export);
        assert!(checked.is_ok(), "{export}: {checked:?}");
    }
}

#[tokio::test]
async fn a_title_to_recognise_reaches_the_export_and_its_ids_come_back() {
    let plugin = recogniser(&["recognise_title"]);
    let title = TitleToRecognise {
        kind: TitleKind::TITLE_KIND_SHOW.into(),
        title: "Show".to_owned(),
        paths: vec!["Show/Show - 137.mkv".to_owned()],
        ..TitleToRecognise::default()
    };

    let answer = plugin.recognise_title(&title).await.unwrap();

    assert_eq!(answer.ids, [tvdb("81189")]);
}

#[tokio::test]
async fn a_file_to_recognise_reaches_the_export_and_its_answer_comes_back() {
    let plugin = recogniser(&["recognise_file"]);
    let file = FileToRecognise {
        path: "Show/Show - 137.mkv".to_owned(),
        library_kind: LibraryKind::LIBRARY_KIND_SHOWS.into(),
        ..FileToRecognise::default()
    };

    let answer = plugin.recognise_file(&file).await.unwrap();

    // The export wraps its request, so the path comes back as the Movie's title and the
    // Library's kind as its year.
    let Some(Recognised::Movie(movie)) = answer.recognised else {
        panic!("{answer:?}");
    };
    assert_eq!(movie.title, "Show/Show - 137.mkv");
    assert_eq!(movie.year, Some(2));
}

#[tokio::test]
async fn a_recogniser_without_the_export_is_not_asked_and_cant_tell() {
    // The module has both exports, so an answer with anything in it means it was asked.
    let plugin = recogniser(&["recognise_file"]);

    let answer = plugin
        .recognise_title(&TitleToRecognise::default())
        .await
        .unwrap();

    assert!(!plugin.recognises(RecogniserMethod::Title));
    assert!(plugin.recognises(RecogniserMethod::File));
    assert!(answer.ids.is_empty(), "{answer:?}");
}
