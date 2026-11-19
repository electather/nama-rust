import { AudioItem } from "@/features/player/components/audio-item";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { RadioGroup } from "@/shared/ui/radio-group";

/** The audio Tracks the plan offers, by their places in `tracks`, the one at `playing` chosen. */
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
    <RadioGroup
      aria-label="Audio"
      value={playing}
      onValueChange={(place) => {
        if (typeof place === "number") {
          choose(place);
        }
      }}
    >
      {audio.map((place) => (
        <AudioItem key={place} place={place} track={tracks[place]} />
      ))}
    </RadioGroup>
  );
}
