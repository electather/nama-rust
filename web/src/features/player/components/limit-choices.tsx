import { RadioGroup, RadioRow } from "nama-ui/components/radio-group";

import { ChoiceLabel } from "@/features/player/components/choice-label";
import type { limitItems } from "@/features/player/lib/limit-items";
import type { PlaybackLimit } from "@/gen/nama/v1/playback_pb";

/** The Playback Limits to pick from, the one `playing` chosen. */
export function LimitChoices({
  items,
  playing,
  choose,
}: ReturnType<typeof limitItems> & { choose: (limit: PlaybackLimit) => void }) {
  return (
    <RadioGroup
      aria-label="Playback Limit"
      value={playing.limit}
      onValueChange={(value) => {
        const picked = items.find(({ limit }) => limit === value);
        if (picked) {
          choose(picked.limit);
        }
      }}
    >
      {items.map(({ limit, name, details }) => (
        <RadioRow key={limit} value={limit}>
          <ChoiceLabel name={name} details={details} />
        </RadioRow>
      ))}
    </RadioGroup>
  );
}
