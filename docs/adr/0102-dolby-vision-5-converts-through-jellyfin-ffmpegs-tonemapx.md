# Dolby Vision 5 converts through jellyfin-ffmpeg's tonemapx

A Dolby Vision 5 Track has no Base Layer, so only its RPU turns the picture into colours any other player can show. The Server Transcodes it with `tonemapx=tonemap=bt2390:peak=100:apply_dovi=1` on frames decoded in software, a filter only jellyfin-ffmpeg has. The Server finds it by `ffmpeg -h filter=tonemapx` listing `apply_dovi`. Without it, a Dolby Vision 5 Version is still refused a Transcode (`DOLBY_VISION_NEEDS_BASE_LAYER`), as [ADR 0026](0026-ffmpeg-comes-from-the-system-not-the-release.md) has the Server use whatever ffmpeg the admin's system has. Both CI test jobs run jellyfin-ffmpeg's portable builds, so CI tests the ffmpeg the Docker image ships (#1415, `docs/research/silo-comparison.md` §4).

## Considered Options

- **Upstream ffmpeg's `libplacebo`**, the only reshaper upstream has: rejected for now. It needs a Vulkan device, and the Docker image's jellyfin-ffmpeg carries only the AMD and Intel Vulkan drivers, with nothing that runs Vulkan on the CPU, so most Servers couldn't use it.
- **jellyfin-ffmpeg's GPU filters** (`tonemap_cuda`, `tonemap_opencl`), as Silo uses beside `tonemapx`: later, once 4K throughput on the CPU is measured. OpenCL on Intel would also need Intel's compute runtime in the image.
- **Tone-map in hardware decode**: rejected. `hevc_qsv` drops the RPU, and `tonemapx` needs 10-bit frames in memory anyway.
- **Keep CI on upstream ffmpeg and skip the Dolby Vision 5 tests there**: rejected. The tests would run only on a developer's machine that happens to have jellyfin-ffmpeg.

## Consequences

- `peak=100` pins the source peak at 1000 nits. Read from the RPU instead, a title mastered at 4000 nits comes out about a third darker in the midtones (Silo found this).
- HDR10 and HLG tone-map through the same `tonemapx` settings when it's there, so every source looks alike. Upstream's zscale and Hable chain stays for other ffmpeg builds.
- The tests use a small committed Dolby Vision 5 clip, made once with x265 from dovi_tool's MIT-licensed `profile5.bin`, because ffmpeg alone can't make one.
