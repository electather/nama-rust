import { AudioLines } from "lucide-react";
import type { RefObject } from "react";

import { AudioChoices } from "@/features/player/components/audio-choices";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { Button } from "@/shared/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/shared/ui/dropdown-menu";

/**
 * Picks which of the Version's audio Tracks plays, by its place in `tracks`, with `choose`, from
 * those `offered`. Absent when there's only one to play.
 */
export function AudioMenu({
  tracks,
  offered: audio,
  playing,
  choose,
  container,
}: {
  tracks: Track[];
  /** The places of the audio Tracks the plan offers. */
  offered: number[];
  /** The place of the one the plan plays. */
  playing: number | undefined;
  choose: (place: number) => void;
  /** Where the menu opens: inside the player, so it shows in full screen. */
  container: RefObject<HTMLDivElement | null>;
}) {
  if (audio.length < 2) {
    return null;
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="overlay" size="icon" aria-label="Audio" />}>
        <AudioLines aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <AudioChoices tracks={tracks} audio={audio} playing={playing} choose={choose} />
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
