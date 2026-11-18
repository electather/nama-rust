import { HardwareSection } from "@/features/admin-playback/components/hardware-section";
import { MissingFromFfmpeg } from "@/features/admin-playback/components/missing-from-ffmpeg";
import { PlaybackSettingsForm } from "@/features/admin-playback/components/playback-settings-form";
import { ProgramRow } from "@/features/admin-playback/components/program-row";
import { usePlaybackStatus } from "@/features/admin-playback/data/use-playback-status";
import { AdminList } from "@/shared/components/admin-list/admin-list";
import { Page } from "@/shared/components/page";

/** The Playback page: what the Server found to Remux and Transcode with, and its Playback settings. */
export function Playback() {
  const status = usePlaybackStatus();
  const { ffmpeg, ffprobe, ffmpegProblem, ffprobeProblem } = status;
  return (
    <Page heading="Playback">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        The Server Direct Plays a Version as it is. To Remux or Transcode one for a Device that
        can’t play it, it needs ffmpeg and ffprobe, which it looks for when it starts: at the paths
        set in its config file, or else on the PATH.
      </p>
      {ffmpeg === undefined || ffprobe === undefined ? (
        <p className="mt-4 max-w-prose text-pretty">
          Without them it can only Direct Play. Install ffmpeg, which comes with ffprobe, or set
          their paths in the config file, then restart the Server.
        </p>
      ) : (
        <MissingFromFfmpeg status={status} />
      )}
      <AdminList>
        <ProgramRow name="ffmpeg" found={ffmpeg} problem={ffmpegProblem} />
        <ProgramRow name="ffprobe" found={ffprobe} problem={ffprobeProblem} />
      </AdminList>
      <HardwareSection status={status} />
      <PlaybackSettingsForm />
    </Page>
  );
}
