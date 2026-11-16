use std::sync::Arc;

use nama_plugin_api::{
    Registrar, Registration,
    blob_store::BlobStore,
    contract::{BlobKey, BlobToPut, BlobsAsked},
};
use nama_plugin_proto::{
    nama::plugin::v1::{Manifest, Point},
    services,
};

use super::super::{
    Refused, Running, check_exports, compile, load::write_cache_config, points, services::POINTS,
};

/// A blob store in WASM's text format, speaking Extism's guest ABI by hand. Each export answers a
/// header from its data followed by its request from some byte on, so what it answers shows what
/// it was asked: `blob_get` turns a `BlobKey` into a found `BlobGot` holding the key as its bytes
/// (field 1's string becomes field 2's bytes), `blob_list` hands a `BlobsAsked` back as a
/// `BlobPage` (its prefix as the one key, its page token as the next), and `blob_exists` answers
/// that it exists. `blob_put` and `blob_delete` answer an empty message.
const BLOB_STORE: &str = r#"
(module
  (import "extism:host/env" "input_length" (func $input_length (result i64)))
  (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
  (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
  (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))
  (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
  (memory 1)
  ;; found: true, then field 2's tag; `blob_exists` takes the first two bytes, exists: true.
  (data (i32.const 0) "\08\01\12")
  ;; Answers the `$head` bytes of the header, then the request from byte `$skip` on.
  (func $answer (param $head i64) (param $skip i64) (result i32)
    (local $length i64) (local $rest i64) (local $out i64) (local $i i64)
    (local.set $length (call $input_length))
    (local.set $rest
      (select
        (i64.sub (local.get $length) (local.get $skip))
        (i64.const 0)
        (i64.gt_u (local.get $length) (local.get $skip))))
    (local.set $out (call $alloc (i64.add (local.get $head) (local.get $rest))))
    (block $done
      (loop $copy
        (br_if $done (i64.ge_u (local.get $i) (local.get $head)))
        (call $store_u8
          (i64.add (local.get $out) (local.get $i))
          (i32.load8_u (i32.wrap_i64 (local.get $i))))
        (local.set $i (i64.add (local.get $i) (i64.const 1)))
        (br $copy)))
    (local.set $i (i64.const 0))
    (block $done
      (loop $copy
        (br_if $done (i64.ge_u (local.get $i) (local.get $rest)))
        (call $store_u8
          (i64.add (local.get $out) (i64.add (local.get $head) (local.get $i)))
          (call $input_load_u8 (i64.add (local.get $skip) (local.get $i))))
        (local.set $i (i64.add (local.get $i) (i64.const 1)))
        (br $copy)))
    (call $output_set (local.get $out) (i64.add (local.get $head) (local.get $rest)))
    (i32.const 0))
  (func $blob_put (result i32) (i32.const 0))
  (func $blob_get (result i32) (call $answer (i64.const 3) (i64.const 1)))
  (func $blob_delete (result i32) (i32.const 0))
  (func $blob_list (result i32) (call $answer (i64.const 0) (i64.const 0)))
  (func $blob_exists (result i32) (call $answer (i64.const 2) (call $input_length)))
  (export "blob_put" (func $blob_put))
  (export "blob_get" (func $blob_get))
  (export "blob_delete" (func $blob_delete))
  (export "blob_list" (func $blob_list))
  (export "blob_exists" (func $blob_exists)))
"#;

const EXPORTS: [&str; 5] = [
    "blob_put",
    "blob_get",
    "blob_delete",
    "blob_list",
    "blob_exists",
];

/// [`BLOB_STORE`] as the Kernel would hold it, registered through its `POINTS` row.
fn blob_store() -> Arc<dyn BlobStore> {
    let data_dir = tempfile::tempdir().unwrap();
    let cache_config = write_cache_config(data_dir.path()).unwrap();
    let running = Running {
        package: Arc::default(),
        compiled: Arc::new(
            compile(BLOB_STORE.as_bytes(), &cache_config, Vec::new(), Vec::new()).unwrap(),
        ),
        secrets: Vec::new().into(),
        crashes: Arc::default(),
        exports: Arc::new(EXPORTS.into_iter().collect()),
    };
    let point = POINTS
        .iter()
        .find(|point| point.service == services::BLOB_STORE)
        .unwrap();
    let mut registrar = Registrar::new();
    (point.register)(&mut registrar, running);
    registrar
        .into_registrations()
        .iter()
        .find_map(Registration::provider::<dyn BlobStore>)
        .unwrap()
}

fn key(key: &str) -> BlobKey {
    BlobKey {
        key: key.to_owned(),
        ..BlobKey::default()
    }
}

fn manifest() -> Manifest {
    Manifest {
        points: vec![Point {
            service: services::BLOB_STORE.to_owned(),
            version: "^1.0.0".to_owned(),
            ..Point::default()
        }],
        ..Manifest::default()
    }
}

#[test]
fn a_manifest_naming_the_blob_store_service_loads_with_every_export() {
    let package = manifest();
    let named = points(&package).unwrap();

    let checked = check_exports(&named, |has| EXPORTS.contains(&has));

    assert!(checked.is_ok(), "{checked:?}");
}

#[test]
fn a_blob_store_missing_any_export_is_refused() {
    let package = manifest();
    let named = points(&package).unwrap();

    for missing in EXPORTS {
        let checked = check_exports(&named, |has| has != missing && EXPORTS.contains(&has));

        assert!(
            matches!(
                checked,
                Err(Refused::MissingExport { service: services::BLOB_STORE, export })
                    if export == missing
            ),
            "{missing}: {checked:?}"
        );
    }
}

#[tokio::test]
async fn a_key_asked_for_reaches_the_export_and_its_object_comes_back() {
    let store = blob_store();

    let got = store.get(&key("artwork/ab/cd.jpg")).await.unwrap();

    assert!(got.found);
    assert_eq!(got.bytes, b"artwork/ab/cd.jpg");
}

#[tokio::test]
async fn a_listing_asked_for_reaches_the_export_and_its_page_comes_back() {
    let store = blob_store();
    let asked = BlobsAsked {
        prefix: "custom-images/".to_owned(),
        page_token: "next".to_owned(),
        ..BlobsAsked::default()
    };

    let page = store.list(&asked).await.unwrap();

    assert_eq!(page.keys, ["custom-images/"]);
    assert_eq!(page.next_page_token, "next");
}

#[tokio::test]
async fn a_put_a_delete_and_an_exists_come_back_answered() {
    let store = blob_store();
    let blob = BlobToPut {
        key: "plugin-logos/s3.svg".to_owned(),
        bytes: b"<svg/>".to_vec(),
        ..BlobToPut::default()
    };

    store.put(&blob).await.unwrap();
    store.delete(&key("plugin-logos/s3.svg")).await.unwrap();
    let exists = store.exists(&key("plugin-logos/s3.svg")).await.unwrap();

    assert!(exists.exists);
}
