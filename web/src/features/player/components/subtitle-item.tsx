import { ChoiceLabel } from "@/features/player/components/choice-label";
import type { SubtitleChoice } from "@/features/player/lib/asking";
import { trackLabel } from "@/features/player/lib/track-label";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { metadataLine } from "@/shared/lib/title-facts";
import { RadioRow } from "@/shared/ui/radio-group";

/** `track`'s row, choosing `value`, with `note` said after its details. */
export function SubtitleItem({
  track,
  value,
  note,
}: {
  track: Track | undefined;
  value: SubtitleChoice;
  note: string;
}) {
  const { name, details } = trackLabel(track);
  return (
    <RadioRow value={value}>
      <ChoiceLabel name={name} details={metadataLine(details, note)} />
    </RadioRow>
  );
}
