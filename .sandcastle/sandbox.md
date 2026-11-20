The sandbox has what CI's checks need, ffmpeg and ffprobe included. Keep it and its build as they are:

- Install nothing and download no tools. If a check needs something the sandbox lacks, say so in your evidence and leave that check to CI.
- One checkout, one `target/`. Don't point another worktree or checkout at it (`CARGO_TARGET_DIR`), and don't `cargo clean` the workspace: a build from scratch takes longer than the factory waits.
- Run commands in the foreground. The factory stops an agent that prints nothing for 30 minutes, and waiting on a background command prints nothing.
