import { MissingFromFfmpeg } from "@/features/admin-playback/components/missing-from-ffmpeg";
import { ProgramRow } from "@/features/admin-playback/components/program-row";
import type { GetPlaybackStatusResponse } from "@/gen/nama/v1/server_pb";

/** The ffmpeg and ffprobe the Server found to Remux and Transcode with, and what it can't do without them. */
export function Programs({ status }: { status: GetPlaybackStatusResponse }) {
  const { ffmpeg, ffprobe, ffmpegProblem, ffprobeProblem } = status;
  return (
    <>
      <p className="max-w-prose text-sm text-pretty text-muted-foreground">
        The Server Direct Plays a Version as it is. To Remux or Transcode one for a Device that
        can’t play it, it needs ffmpeg and ffprobe, which it looks for when it starts: at the paths
        set in its config file, or else on the PATH.
      </p>
      {ffmpeg === undefined || ffprobe === undefined ? (
        <p className="mt-4 max-w-prose text-sm text-pretty">
          Without them it can only Direct Play. Install ffmpeg, which comes with ffprobe, or set
          their paths in the config file, then restart the Server.
        </p>
      ) : (
        <MissingFromFfmpeg status={status} />
      )}
      <ul className="-mx-3 mt-4 flex flex-col">
        <ProgramRow name="ffmpeg" found={ffmpeg} problem={ffmpegProblem} />
        <ProgramRow name="ffprobe" found={ffprobe} problem={ffprobeProblem} />
      </ul>
    </>
  );
}
