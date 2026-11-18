import type { GetPlaybackStatusResponse } from "@/gen/nama/v1/server_pb";

/** What the Server's ffmpeg lacks that a Transcode needs, if anything. */
export function MissingFromFfmpeg({ status }: { status: GetPlaybackStatusResponse }) {
  if (!status.canTranscode && !status.hasLibx264) {
    return (
      <p className="mt-4 max-w-prose text-pretty">
        This Server’s ffmpeg lacks libx264, so it can’t Transcode, only Direct Play and Remux.
        Install an ffmpeg built with libx264, such as jellyfin-ffmpeg, which the Docker image comes
        with, then restart the Server.
      </p>
    );
  }
  if (!status.hasZscale) {
    return (
      <p className="mt-4 max-w-prose text-pretty">
        This Server’s ffmpeg lacks zscale, so a Transcode can’t tone-map HDR to SDR in software, and
        HDR looks washed out on an SDR screen unless the GPU tone-maps it. Install an ffmpeg built
        with zscale, such as jellyfin-ffmpeg, then restart the Server.
      </p>
    );
  }
  return null;
}
