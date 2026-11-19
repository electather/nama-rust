import type { SubtitleChoice } from "@/features/player/lib/asking";
import type { Track } from "@/gen/nama/v1/browse_pb";
import type { PlaybackPlan } from "@/gen/nama/v1/playback_pb";

/** A frame's length when the file doesn't say its frame rate: film's, 24 a second. */
const FALLBACK_FRAME_SECONDS = 1 / 24;

/** How long one frame of the Version's video shows, in seconds, for `,` and `.` to step by. */
export function frameSeconds(tracks: Track[]) {
  for (const { kind } of tracks) {
    if (kind.case === "video" && kind.value.frameRate > 0) {
      return 1 / kind.value.frameRate;
    }
  }
  return FALLBACK_FRAME_SECONDS;
}

/**
 * What C chooses: Off while a subtitle shows or is burned in, or else the one the plan started
 * with, or else its first. Only ones the Device shows itself, so C never asks for a new plan to
 * turn one on.
 */
export function captionsToggled(plan: PlaybackPlan, shown: string | undefined): SubtitleChoice {
  return (shown ?? plan.subtitleTrack) === undefined ? firstSubtitle(plan) : undefined;
}

function firstSubtitle({ subtitles, startingSubtitle }: PlaybackPlan) {
  return (subtitles.find(({ place }) => place === startingSubtitle) ?? subtitles[0])?.url;
}
