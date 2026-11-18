import { AudioItem } from "@/features/player/components/audio-item";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { DropdownMenuRadioGroup } from "@/shared/ui/dropdown-menu";

/** The audio Tracks the plan offers, by their places in `tracks`, the one at `playing` checked. */
export function AudioChoices({
  tracks,
  audio,
  playing,
  choose,
}: {
  tracks: Track[];
  audio: number[];
  playing: number | undefined;
  choose: (place: number) => void;
}) {
  return (
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
  );
}
