## ffmpeg

Finding the system's ffmpeg and ffprobe, asking what they can do, and building the commands that Remux, Transcode and probe Versions.

- **What ffmpeg can do is asked once.** Only `Abilities` in `abilities.rs` asks the Server's ffmpeg whether it runs, has zscale, `tonemapx` with `apply_dovi`, `alphasrc` with `subtitles`' `sub2video`, or libx264, and which hardware passes its test on which GPU, and remembers each answer; whether it has those, and which hardware passes when it couldn't list its encoders, is asked again while ffmpeg couldn't say. Planning and the `GetPlaybackStatus` and `UpdatePlaybackSettings` handlers read the one `CanDo` it hands back, and ask its methods what follows: whether a Transcode runs, what tone-maps HDR on each hardware, and which Automatic picks.
- **It alone reads ffmpeg's and ffprobe's words.** Only this area recognises what ffmpeg or ffprobe print on stderr, such as `RefusedWrite` in `refused_write.rs` for a write the folder refused; a caller asks it, so a new phrase is added once, here.
- **Only this area starts ffmpeg or ffprobe with a timeout.** A neighbour reads with `Ffmpeg::run` and wraps its `RunError`, the one error for how either program failed.
- **`fake` stays addressed by name.** The crate's "Fake programs" rule sends every test's fake program through `ffmpeg::fake::script`, and neighbours' tests share the one two-GPU VAAPI fake, `fake::vaapi_on_two_gpus_at`, so `fake` stays a public module they reach by that path, the one entry `SURFACE_EXCEPTIONS` lists for this area.

### Neighbours

- Uses: `config`.
- Used by: `playback` (with tracks and probing), `server_settings` (`Accel`, `Hardware`), other areas' tests (`fake::script`, `fake::vaapi_on_two_gpus` and `fake::vaapi_on_two_gpus_at`), the `api` parts for playback and the Server, and startup in `lib.rs` and `main.rs`.
