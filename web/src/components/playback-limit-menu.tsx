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
import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";

/** The Playback Limits below Original, largest first. */
export const belowOriginal = [
  { limit: PlaybackLimit.PLAYBACK_LIMIT_1080P, name: "1080p" },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_720P, name: "720p" },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_480P, name: "480p" },
];

/**
 * Picks the Playback Limit with `choose`: Original, or one of those `offered` below it. Absent when
 * none is offered.
 */
export function PlaybackLimitMenu({
  offered,
  chosen,
  choose,
  container,
}: {
  /** The limits below Original the plan offers. */
  offered: PlaybackLimit[];
  /** The limit the plan keeps to. */
  chosen: PlaybackLimit;
  choose: (limit: PlaybackLimit) => void;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  const below = belowOriginal.filter(({ limit }) => offered.includes(limit));
  if (below.length === 0) return null;
  const original = PlaybackLimit.PLAYBACK_LIMIT_ORIGINAL;
  const items = [{ limit: original, name: "Original" }, ...below];
  // A limit the picture already fits, or none, plays it as it is.
  const playing = items.some(({ limit }) => limit === chosen) ? chosen : original;
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
            const picked = items.find(({ limit }) => String(limit) === value);
            if (picked) choose(picked.limit);
          }}
        >
          {items.map(({ limit, name }) => (
            <DropdownMenuRadioItem key={limit} value={String(limit)}>
              {name}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
