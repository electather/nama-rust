import { type SeasonAsking, askFor } from "@/features/title/hooks/season-asking";
import { RequestSplitButton } from "@/shared/components/request-button/request-split-button";
import type { Standing } from "@/shared/lib/request-standing";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * "Request Season N", the page's primary action, while its `standing` lets you ask for `season`. Its
 * Quality may be picked only while it opens a Request and the Show has none.
 */
export function SeasonRequestButton({
  asking,
  season,
  standing,
}: {
  asking: SeasonAsking;
  season: number;
  standing: Standing;
}) {
  const name = seasonName(season);
  return (
    standing.can !== "nothing" && (
      <RequestSplitButton
        what={`${name} of ${asking.show.title}`}
        label={`Request ${name}`}
        kind="show"
        opening={standing.can === "open" && asking.show.quality === undefined}
        primary
        busy={asking.asking.isPending}
        onRequest={(quality) => {
          askFor(asking, [season], quality);
        }}
      />
    )
  );
}
