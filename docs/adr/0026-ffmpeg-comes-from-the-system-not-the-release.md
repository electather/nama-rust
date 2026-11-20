# ffmpeg comes from the system, not the release

The Server Remuxes, Transcodes and probes Versions with ffmpeg and ffprobe, but its releases don't include them. It uses the ones on `PATH`, or at a path set in its config, and checks their version at startup. Without them it still runs and only Direct Plays, and it tells the admin why. A configured bare program name with no folder, such as `ffmpeg` or `jellyfin-ffmpeg`, is looked up on `PATH` the way a shell would. Any other relative path, such as `./ffmpeg` or `bin/ffmpeg`, counts from the folder the Server started in. The Docker image is where a known-good ffmpeg with hardware acceleration is shipped, so that image moves from M5 into M4.

## Considered Options

- **Bundle ffmpeg in every release**: rejected. It adds about 80 MB to each of five platform builds, ties ffmpeg's GPL/LGPL licensing to the release, and a static build still can't reach every GPU driver an admin's system has.
- **Every relative configured path, even a bare name, counts from the start folder**: rejected. An admin who writes `ffmpeg = "ffmpeg"` means the one on `PATH`, not a file in whatever folder the Server started from, and there'd be no way to pick a differently named build on `PATH` without its full path. `./ffmpeg` still says "this folder" plainly.
- **Download a known build, such as jellyfin-ffmpeg, the first time it's needed**: rejected. The Server would fetch and run code at runtime, there's no build for every target, and an offline Server couldn't Transcode at all.

## Consequences

- Which hardware acceleration works depends on the admin's ffmpeg build, so the Server finds out what its ffmpeg can actually do instead of assuming.
- Probing needs ffprobe too, so a Server without it stores no Tracks and offers Direct Play only.
