import { AudioLines } from "lucide-react";
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
import { metadataLine, soundOf } from "@/lib/browse";
import { trackLabel } from "@/lib/subtitles";

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
  if (audio.length < 2) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="overlay" size="icon" aria-label="Audio" />}>
        <AudioLines aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuRadioGroup
          value={String(playing)}
          onValueChange={(place: string) => {
            choose(Number(place));
          }}
        >
          {audio.map((place) => (
            <AudioItem key={place} place={place} track={tracks[place]} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function AudioItem({ place, track }: { place: number; track: Track | undefined }) {
  const { name, details } = trackLabel(track);
  const more = metadataLine(details, track && soundOf([track]));
  return (
    <DropdownMenuRadioItem value={String(place)}>
      {name}
      {more !== "" && <span className="text-muted-foreground"> {more}</span>}
    </DropdownMenuRadioItem>
  );
}
