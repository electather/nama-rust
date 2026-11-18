import type { SubtitleChoice } from "@/features/player/lib/asking";
import { trackLabel } from "@/features/player/lib/track-label";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { DropdownMenuRadioItem } from "@/shared/ui/dropdown-menu";

/** `track`'s item, choosing `value`, with `note` said after its details. */
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
  const more = [details, note].filter((text) => text !== "").join(" · ");
  return (
    <DropdownMenuRadioItem value={value}>
      {name}
      {more !== "" && <span className="text-muted-foreground"> {more}</span>}
    </DropdownMenuRadioItem>
  );
}
