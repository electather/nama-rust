import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { belowOriginal } from "@/shared/lib/playback-names";

/** The Playback Limits an Admin can pick from, as the player's menu names them. */
export const limits = [
  { limit: PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL, name: "Original" },
  ...belowOriginal,
];

/** The Playback Limit an `<option>`'s `value` names, or else Original. */
export function limitPicked(value: string) {
  const picked = limits.find(({ limit }) => String(limit) === value);
  return picked?.limit ?? PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL;
}
