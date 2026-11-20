import { useState } from "react";

import { useSetSubtitleOffset } from "@/features/player/data/use-set-subtitle-offset";
import type { PlaybackSubtitle } from "@/gen/nama/v1/playback_pb";

/**
 * `offsetOf` a plan's subtitle: the User's Subtitle Offset for its Track, as they set it since the
 * player opened, or else as the plan says. `shiftSubtitle` sets it and saves it for that Track.
 */
export function useSubtitleOffsets(versionId: bigint) {
  // By place, which every plan for the Version gives its Tracks.
  const [set, setSet] = useState<Partial<Record<number, number>>>({});
  const save = useSetSubtitleOffset(versionId);
  return {
    offsetOf: ({ place, offsetMs }: PlaybackSubtitle) => set[place] ?? offsetMs,
    shiftSubtitle: ({ place }: PlaybackSubtitle, offsetMs: number) => {
      setSet((before) => ({ ...before, [place]: offsetMs }));
      save(place, offsetMs);
    },
  };
}
