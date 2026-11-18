import { SubtitleItem } from "@/features/player/components/subtitle-item";
import type { Track } from "@/gen/nama/v1/browse_pb";

function isImageSubtitle(track: Track | undefined) {
  return track?.kind.case === "subtitle" && track.kind.value.image;
}

/** The item burning in `track`, at `place` in the Version's Tracks. */
export function BurnInItem({ track, place }: { track: Track | undefined; place: number }) {
  // An image one has no styling to tell it from.
  const note = isImageSubtitle(track) ? "" : "With styling";
  return <SubtitleItem track={track} value={place} note={note} />;
}
