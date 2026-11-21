use super::RefusedWrite;

#[test]
fn stderr_says_whether_and_why_a_write_was_refused() {
    let cases = [
        (
            "av_interleaved_write_frame(): No space left on device",
            Some(RefusedWrite::Full),
        ),
        ("[mp4] 0.m4s: Disk quota exceeded", Some(RefusedWrite::Full)),
        (
            "Error writing trailer: Read-only file system",
            Some(RefusedWrite::ReadOnly),
        ),
        ("movie.mkv: Invalid data found when processing input", None),
    ];
    for (stderr, refused) in cases {
        assert_eq!(RefusedWrite::in_stderr(stderr), refused, "{stderr}");
    }
}
