//! The migration numbers the Server applies, read from `migrations.txt` so a new migration edits one file.

/// The number each line of `migrations.txt` starts with, in order: `0054_subtitle_headers.sql` gives 54.
#[expect(
    clippy::expect_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) fn numbers() -> Vec<i64> {
    include_str!("../../migrations.txt")
        .lines()
        .map(|line| {
            line.split('_')
                .next()
                .and_then(|number| number.parse().ok())
                .expect("every line of migrations.txt starts with its number")
        })
        .collect()
}
