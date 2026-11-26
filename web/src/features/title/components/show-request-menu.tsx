import { MoreActions } from "@/features/title/components/more-actions";
import { RequestSeasonsItems } from "@/features/title/components/request-seasons-items";
import { useSeasonAsking } from "@/features/title/hooks/season-asking";
import { seasonsToAsk } from "@/features/title/lib/seasons-to-ask";
import type { RowEpisode } from "@/features/title/lib/show-row";

/**
 * A found Show's "…", which Requests many of the Seasons of its `row` at once, shown only while
 * there's one to ask for.
 */
export function ShowRequestMenu({ row }: { row: RowEpisode[] }) {
  const asking = useSeasonAsking();
  return (
    asking &&
    seasonsToAsk(asking.show, asking.you).length > 0 && (
      <MoreActions label="More actions">
        <RequestSeasonsItems row={row} />
      </MoreActions>
    )
  );
}
