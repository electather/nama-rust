import { pictureWords } from "@/features/player/lib/track-change";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PlaybackLimit, type PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { belowOriginal } from "@/shared/lib/playback-names";

/**
 * The Playback Limits `plan` offers: Original, with the Version's picture, such as "4K HDR10", then
 * those below it, and the one it plays at. None when it offers none below Original.
 */
export function limitItems(plan: PlaybackPlan, tracks: Track[]) {
  const below = belowOriginal.filter(({ limit }) => plan.playbackLimits.includes(limit));
  const original = {
    limit: PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL,
    name: "Original",
    details: pictureLine(tracks),
  };
  const items =
    below.length === 0
      ? []
      : [original, ...below.map(({ limit, name }) => ({ limit, name, details: "" }))];
  // A limit the picture already fits, or none, plays it as it is.
  const playing = items.find(({ limit }) => limit === plan.playbackLimit) ?? original;
  return { items, playing };
}

function pictureLine(tracks: Track[]) {
  const video = tracks.find(({ kind }) => kind.case === "video");
  return video === undefined ? "" : pictureWords(video);
}
