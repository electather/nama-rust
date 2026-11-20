# S3

An Installed Plugin, bundled in the Server, that serves the blob store point (ADR [0052](../../docs/adr/0052-the-kernels-stored-files-live-in-a-blob-store.md)): it keeps the Server's stored files, such as its images' originals, Custom Images and Plugin logos, in an S3 or S3-compatible bucket. It never holds media files.

It signs each request with AWS Signature V4 in the request's URL, through [`rusty-s3`](https://crates.io/crates/rusty-s3), at the time `nama_pdk::now` gives. Testing the settings lists one key of the bucket, so a wrong key or secret shows there.

## Settings

- **Endpoint**: the store's S3 API, such as `https://s3.eu-west-1.amazonaws.com`.
- **Region**: the bucket's region, `us-east-1` unless set.
- **Bucket**: the bucket the files go in.
- **Prefix**: put before every key, such as `nama/`.
- **Access key ID** and **Secret access key**: a key that may read, write, delete and list the bucket. Without both, requests go unsigned.
- **Path-style addressing**: reach the bucket at `<endpoint>/<bucket>` rather than `<bucket>.<endpoint's host>`.

An S3-compatible store, such as MinIO, needs path-style addressing. A Plugin may reach only the hosts its Manifest names, which here is `*.amazonaws.com`, and the endpoint's own host. A virtual-hosted bucket on any other store is at a host neither covers.

## Tests

`src/lib_tests.rs` calls the blob store's methods through `nama_pdk::testing`, answered by `src/fake_s3.rs`, an S3 bucket in memory that checks each request's signature with the secret it was given, so the tests need no network and no credentials.
