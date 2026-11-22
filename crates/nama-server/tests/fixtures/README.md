# Test fixtures

Tests make the media they need with ffmpeg (`src/ffmpeg/test_media.rs`), except for what ffmpeg alone can't make.

## `dolby-vision-5.mp4`

A second of 640×360 Dolby Vision profile 5 HEVC at 24 fps, with no Base Layer (`dv_profile=5`, `dv_bl_signal_compatibility_id=0`), for the tests of converting it (ADR 0102). Its RPU, on every frame, is [dovi_tool](https://github.com/quietvoid/dovi_tool)'s `assets/tests/profile5.bin`, © quietvoid, under the MIT License.

ffmpeg can't write a Dolby Vision RPU of its own, and `dovi_tool generate` makes only profile 8.1 or 8.4, so it was made with ffmpeg 8.1.3 (libx265 4.x) and a few lines of Python that put the RPU after each frame:

1. Encode 10-bit HEVC with an access unit delimiter before each frame:
   `ffmpeg -f lavfi -i testsrc2=s=640x360:r=24:d=1 -pix_fmt yuv420p10le -c:v libx265 -x265-params aud=1 -f hevc bl.hevc`
2. Append `profile5.bin` as an RPU NAL unit (type 62, header `7C 01`) to each frame:

   ```python
   import sys
   data, rpu = open("bl.hevc", "rb").read(), open("profile5.bin", "rb").read()
   aud = b"\0\0\0\1\x46"  # an access unit delimiter's start code and header
   nal = b"\0\0\0\1\x7c\x01" + rpu[4:]  # the RPU, its start code swapped for a NAL header
   frames = [aud + frame for frame in data.split(aud)[1:]]
   open("p5.hevc", "wb").write(b"".join(frame + nal for frame in frames))
   ```

3. Encode it again with `-dolbyvision 1`, which needs ffmpeg 7.1 or later, so libx265 writes the RPU it decoded and the `dvcC` record:
   `ffmpeg -i p5.hevc -c:v libx265 -dolbyvision 1 -maxrate 5M -bufsize 10M -tag:v dvh1 -strict unofficial dolby-vision-5.mp4`

With the x265 command line, `x265 --dolby-vision-profile 5 --dolby-vision-rpu rpu.bin` does step 2 from `profile5.bin` repeated once per frame.
