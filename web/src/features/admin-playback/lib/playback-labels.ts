import type { PlaybackValues } from "@/features/admin-playback/lib/playback-settings-form";
import type { SettingText } from "@/shared/lib/setting-text";

/**
 * What each setting's field says, in the pane's order, ffmpeg's group first. The toast names a
 * refused change by its label.
 */
export const serverPlaybackLabels: Record<keyof PlaybackValues | "programs", SettingText> = {
  programs: { label: "ffmpeg and ffprobe", words: "transcoding remux direct play" },
  segmentFolder: {
    label: "Segment folder",
    hint: "Where a Remux or Transcode writes the files it plays from, such as a tmpfs. Files further behind than Keep for skipping back are deleted as it plays, and the rest a minute after it’s last played. Leave it empty for the transcodes folder in the Server’s data folder. A change applies from the next one.",
    words: "segments",
  },
  keepBehindMinutes: {
    label: "Keep for skipping back",
    hint: "How much of a Remux or Transcode is kept behind where the player is, so skipping back that far plays at once. Skipping further waits while it’s made again. The kept files take room in the segment folder, memory on a tmpfs, so it must fit them: a 4K Remux keeps about 1 GB for 5 minutes. Leave it empty for 5 minutes. A change applies to those already playing.",
  },
  hardwareAcceleration: {
    label: "Hardware acceleration",
    hint: "What every Transcode decodes, scales and encodes video on. Automatic picks the one marked above. Off runs them in software, and so does a pick that was missing or didn’t pass its test when the Server started. A change applies from the next Transcode.",
    words: "gpu vaapi qsv nvenc videotoolbox transcoding",
  },
  maxTranscodes: {
    label: "Transcodes at once",
    hint: "The most the Server runs at once. Remuxes don’t count. While that many run, a Device that can play a Remux gets one instead, and one that can’t is told the Server is busy. Leave it empty for at most 2 at once of those that run on the CPU, in software or tone-mapping HDR there, and no limit for those that run fully on a GPU.",
    words: "concurrent limit transcoding",
  },
  remotePlaybackLimit: {
    label: "Default Playback Limit for remote playback",
    hint: "The most a Device outside the local network is sent until its User picks a Playback Limit in the player. Original sends each Version as it is.",
    words: "bitrate quality",
  },
};
