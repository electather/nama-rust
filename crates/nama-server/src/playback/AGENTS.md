## Playback

Planning how a Device plays a Version, running the plans, serving their files and subtitles, the Tracks, probing and Track Preferences plans are based on, whether a User's next Episode plays by itself, and their Subtitle Appearance.

- **What a plan converts is worked out once.** Before the steps run, `candidates` in `../playback.rs` works out what a Remux and a Transcode of the Version would convert, and the plan keeps the one it narrowed to in `Playing`, with the Version's length, which a Remux's or a Transcode's playlist lists, and, for a Transcode on hardware, what it falls back to in software and why. Its reasons, Tracks, events, playlist and ffmpeg arguments read that value; none asks the planning facts again what it converts, how long the Version is or what it falls back to.
- **`RunningPlans` knows only running plans.** What ffmpeg can do it reads from the `CanDo` that `Abilities` hands back.
- **One owner for Tracks.** Only `tracks.rs` reads or writes `version_probes`, `tracks` and `probe_failures`, tests included. Browse attaches them to the Versions `Contents` reads.
- **One owner for sidecars.** Only `sidecars.rs` reads or writes `sidecar_subtitles`, tests included. A scan says which sidecars each Version has; their language, flags and title come from their names. Every reader of a Version's Tracks reads them with `StoredTracks::of`, which puts its sidecars after the file's own, so those keep their places.
- **One owner for probing.** Only Probing (`probing.rs`, with its private `probing/ffprobe.rs` and `probing/cues.rs`) runs ffprobe on stored files or reads their Cues, and decides what a probe failure means for trying it again (`ProbeFailure`); Tracks stores it.
- **One owner for Track Preferences.** Only `track_preferences.rs` reads or writes `track_preferences`, tests included, and decides what a Preference is kept for: the Show or Movie a Version is of, keyed as the per-title rule in `crates/nama-server/AGENTS.md` says (ADR 0035).
- **One owner for autoplay.** Only `autoplay.rs` reads or writes `autoplay_off`, tests included.
- **One owner for Subtitle Appearance.** Only `subtitle_appearance.rs` reads or writes `subtitle_appearances`, tests included. It's no plan input, so a Burn-in never takes it (ADR 0068).
- **Probe version.** A change to what probing reads from a file, or how, bumps `PROBE_VERSION` in `tracks.rs`, so the next pass after a scan probes every file again (ADR 0023's pattern).
- **Probe failure version.** A change that may let ffprobe read files it lastingly failed on before, without changing what probing reads from a file that it could read, bumps `PROBE_FAILURE_VERSION` in `tracks.rs`, so the next pass tries again only those files, and the next Remux lists the keyframes it couldn't before.
- **WebVTT version.** A change to the WebVTT a subtitle source makes bumps `WEBVTT_VERSION` in `subtitles.rs`, so what the cache kept from before is made again.

### Neighbours

- Uses: `ffmpeg`, `library`, `server_settings` (its settings and `PlaybackLimit`), `events`, `images`, `users`, `db`, `diagnostics` (the trace a probe pass joins), and `config` and `plugins` in tests.
- Used by: `api`, `browse`, `scanner` and startup in `lib.rs`.
