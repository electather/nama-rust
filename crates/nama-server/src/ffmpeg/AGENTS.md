## ffmpeg

Finding the system's ffmpeg and ffprobe, asking what they can do, and building the commands that Remux, Transcode and probe Versions.

- **What ffmpeg can do is asked once.** Only `Abilities` in `ffmpeg.rs` asks the Server's ffmpeg whether it runs, has zscale or libx264, and which hardware passes its test on which GPU, and remembers each answer; whether it has zscale or libx264, and which hardware passes when it couldn't list its encoders, is asked again while ffmpeg couldn't say. Planning and the `GetPlaybackStatus` and `UpdatePlaybackSettings` handlers read the one `CanDo` it hands back, and ask its methods what follows: whether a Transcode runs, what tone-maps HDR on each hardware, and which Automatic picks. `RunningPlans` knows only running plans.
- **`fake` stays addressed by name.** The crate's "Fake programs" rule sends every test's fake program through `ffmpeg::fake::script`, so `fake` stays a public module neighbours' tests reach by that path.
- **Private modules.** `hardware` is private; neighbours import what `ffmpeg.rs` re-exports (`Accel`, `Gpu`, `Hardware`, `find_gpus`), and items inside default to `pub(super)`.

### Neighbours

- Uses: `config`.
- Used by: `playback` (with tracks and probing), `server_settings` (`Accel`, `Hardware`, `find_gpus`), other areas' tests (`fake::script`), the `api` parts for playback and the Server, and startup in `lib.rs` and `main.rs`.
