//! Build script for the Hub.

fn main() {
    // `sqlx::migrate!` embeds migrations/ at compile time; rebuild when it changes.
    println!("cargo:rerun-if-changed=migrations");
}
