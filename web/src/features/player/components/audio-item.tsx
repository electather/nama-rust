import { trackLabel } from "@/features/player/lib/track-label";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { metadataLine } from "@/shared/lib/title-facts";
import { soundOf } from "@/shared/lib/version-facts";
import { DropdownMenuRadioItem } from "@/shared/ui/dropdown-menu";

/** An audio Track's item, at `place` in the Version's Tracks. */
export function AudioItem({ place, track }: { place: number; track: Track | undefined }) {
  const { name, details } = trackLabel(track);
  const more = metadataLine(details, track && soundOf([track]));
  return (
    <DropdownMenuRadioItem value={String(place)}>
      {name}
      {more !== "" && <span className="text-muted-foreground"> {more}</span>}
    </DropdownMenuRadioItem>
  );
}
