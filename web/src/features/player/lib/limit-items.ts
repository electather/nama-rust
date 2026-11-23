import { pictureWords } from "@/features/player/lib/track-change";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PlaybackLimit, type PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import { belowOriginal } from "@/shared/lib/playback-names";

/**
 * The Playback Limits `plan` offers: Original, with the Version's picture, such as "4K HDR10", then
 * those below it, and the one it plays at. None when it offers none below Original. Those above
 * the User's cap for remote playback are `unavailable`.
 */
export function limitItems(plan: PlaybackPlan, tracks: Track[]) {
  const below = belowOriginal.filter(({ limit }) => plan.playbackLimits.includes(limit));
  const capAt = belowOriginal.findIndex(({ limit }) => limit === plan.remotePlaybackLimit);
  const original = {
    limit: PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL,
    name: "Original",
    details: pictureLine(tracks),
    // Original is above the cap only when the picture is, which is when the cap scales it down.
    unavailable: plan.playbackLimits.includes(plan.remotePlaybackLimit),
  };
  const items =
    below.length === 0
      ? []
      : [
          original,
          ...below.map(({ limit, name }) => ({
            limit,
            name,
            details: "",
            unavailable: belowOriginal.findIndex((each) => each.limit === limit) < capAt,
          })),
        ];
  // A limit the picture already fits, or none, plays it as it is.
  const playing = items.find(({ limit }) => limit === plan.playbackLimit) ?? original;
  return { items, playing };
}

function pictureLine(tracks: Track[]) {
  const video = tracks.find(({ kind }) => kind.case === "video");
  return video === undefined ? "" : pictureWords(video);
}
