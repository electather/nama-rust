import type { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { DropdownMenuRadioGroup, DropdownMenuRadioItem } from "@/shared/ui/dropdown-menu";

/** The Playback Limits to pick from, the one `playing` checked. */
export function LimitChoices({
  items,
  playing,
  choose,
}: {
  items: { limit: PlaybackLimit; name: string }[];
  playing: PlaybackLimit;
  choose: (limit: PlaybackLimit) => void;
}) {
  return (
    <DropdownMenuRadioGroup
      value={String(playing)}
      onValueChange={(value: string) => {
        const picked = items.find(({ limit }) => String(limit) === value);
        if (picked) {
          choose(picked.limit);
        }
      }}
    >
      {items.map(({ limit, name }) => (
        <DropdownMenuRadioItem key={limit} value={String(limit)}>
          {name}
        </DropdownMenuRadioItem>
      ))}
    </DropdownMenuRadioGroup>
  );
}
