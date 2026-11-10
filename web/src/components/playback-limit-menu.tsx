import { Gauge } from "lucide-react";
import type { RefObject } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";

/**
 * The Playback Limits below Original, with the picture each fits a Transcode in, as the Server's
 * `PlaybackLimit` in `crates/nama-server/src/playback.rs` has them.
 */
const belowOriginal = [
  { limit: PlaybackLimit.PLAYBACK_LIMIT_1080P, name: "1080p", width: 1920, height: 1080 },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_720P, name: "720p", width: 1280, height: 720 },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_480P, name: "480p", width: 854, height: 480 },
];

/**
 * Picks the Playback Limit with `choose`: Original, or one below the Version's picture. Absent when
 * none is below it, or its picture size isn't known.
 */
export function PlaybackLimitMenu({
  tracks,
  chosen,
  choose,
  container,
}: {
  /** The Version's Tracks, whose first video Track's picture the limits must be below. */
  tracks: Track[];
  chosen: PlaybackLimit;
  choose: (limit: PlaybackLimit) => void;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  const [video] = tracks.flatMap(({ kind }) => (kind.case === "video" ? [kind.value] : []));
  // As the Server decides: a limit is below a picture wider or taller than it.
  const below = belowOriginal.filter(
    ({ width, height }) => video !== undefined && (video.width > width || video.height > height),
  );
  if (below.length === 0) return null;
  const original = PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL;
  const offered = [{ limit: original, name: "Original" }, ...below];
  // Not picking one is Original.
  const playing = chosen === PlaybackLimit.PLAYBACK_LIMIT_UNSPECIFIED ? original : chosen;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={<Button variant="overlay" size="icon" aria-label="Playback Limit" />}
      >
        <Gauge aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuRadioGroup
          value={String(playing)}
          onValueChange={(value: string) => {
            const picked = offered.find(({ limit }) => String(limit) === value);
            if (picked) choose(picked.limit);
          }}
        >
          {offered.map(({ limit, name }) => (
            <DropdownMenuRadioItem key={limit} value={String(limit)}>
              {name}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
