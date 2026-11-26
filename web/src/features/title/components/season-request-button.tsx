import { type SeasonAsking, askFor } from "@/features/title/hooks/season-asking";
import { opensRequest } from "@/features/title/lib/episode-corner-shown";
import type { Request } from "@/gen/nama/v1/request_pb";
import { RequestSplitButton } from "@/shared/components/request-button/request-split-button";
import { canAsk } from "@/shared/lib/can-ask";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * "Request Season N", the page's primary action, while you can ask for `season` with `request`. Its
 * Quality may be picked only while it opens a Request and the Show has none.
 */
export function SeasonRequestButton({
  asking,
  season,
  request,
}: {
  asking: SeasonAsking;
  season: number;
  request: Request | undefined;
}) {
  const name = seasonName(season);
  return (
    canAsk(request, asking.you) && (
      <RequestSplitButton
        what={`${name} of ${asking.show.title}`}
        label={`Request ${name}`}
        kind="show"
        opening={opensRequest(request) && asking.show.quality === undefined}
        primary
        busy={asking.asking.isPending}
        onRequest={(quality) => {
          askFor(asking, [season], quality);
        }}
      />
    )
  );
}
