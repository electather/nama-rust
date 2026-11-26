import { DropdownMenuItem } from "nama-ui/components/dropdown-menu";

import { askFor, useSeasonAsking } from "@/features/title/hooks/season-asking";
import { seasonsToAsk, unwatchedToAsk } from "@/features/title/lib/seasons-to-ask";
import type { RowEpisode } from "@/features/title/lib/show-row";

/**
 * A Show's "…" items that Request many Seasons at once, at the User's Quality: every numbered one
 * missing that you can ask for, or only those of them you haven't watched any of in `row`. Each
 * shows only while it has a Season to ask for.
 */
export function RequestSeasonsItems({ row }: { row: RowEpisode[] }) {
  const asking = useSeasonAsking();
  if (!asking) {
    return null;
  }
  const all = seasonsToAsk(asking.show, asking.you);
  const unwatched = unwatchedToAsk(asking.show, row, asking.you);
  const disabled = asking.asking.isPending;
  return (
    <>
      {all.length > 0 && (
        <DropdownMenuItem
          disabled={disabled}
          onClick={() => {
            askFor(asking, all);
          }}
        >
          Request All Seasons
        </DropdownMenuItem>
      )}
      {unwatched.length > 0 && (
        <DropdownMenuItem
          disabled={disabled}
          onClick={() => {
            askFor(asking, unwatched);
          }}
        >
          Request Unwatched
        </DropdownMenuItem>
      )}
    </>
  );
}
