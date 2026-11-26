//! The helpers the Server's integration tests share, which the `server` test program declares
//! once. The folder stays at `tests/support/`, where the separate test programs and the wasm unit
//! tests include single files by path.

pub(crate) mod browsing;
pub(crate) mod custom_images;
pub(crate) mod fake_hub;
pub(crate) mod fake_s3;
pub(crate) mod home_layouts;
pub(crate) mod home_row_plugin;
pub(crate) mod http;
pub(crate) mod installed_test_plugin;
pub(crate) mod kept_artwork;
pub(crate) mod log;
pub(crate) mod maximum_age;
pub(crate) mod refused;
pub(crate) mod server;
pub(crate) mod test_plugin;
