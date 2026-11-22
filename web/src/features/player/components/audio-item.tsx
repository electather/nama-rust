import { RadioRow } from "nama-ui/components/radio-group";

import { ChoiceLabel } from "@/features/player/components/choice-label";
import { trackLabel } from "@/features/player/lib/track-label";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { metadataLine } from "@/shared/lib/title-facts";
import { soundOf } from "@/shared/lib/version-facts";

/** An audio Track's row, at `place` in the Version's Tracks. */
export function AudioItem({ place, track }: { place: number; track: Track | undefined }) {
  const { name, details } = trackLabel(track);
  return (
    <RadioRow value={place}>
      <ChoiceLabel name={name} details={metadataLine(details, track && soundOf([track]))} />
    </RadioRow>
  );
}
