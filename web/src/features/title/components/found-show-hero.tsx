import { SeasonAsk } from "@/features/title/components/season-ask";
import { SeasonAskRefusal } from "@/features/title/components/season-ask-refusal";
import { ShowRequestMenu } from "@/features/title/components/show-request-menu";
import { TitleTop } from "@/features/title/components/title-top";
import { heroTarget } from "@/features/title/lib/hero-target";
import type { RowEpisode } from "@/features/title/lib/show-row";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { FoundShow } from "@/gen/nama/v1/request_pb";

/**
 * The top of a found Show's page: what it is, "Request Season N" for the first Season of its `row`
 * missing from the Server, or where its Request is, and a "…" that Requests many.
 */
export function FoundShowHero({
  show,
  showId,
  row,
}: {
  show: FoundShow;
  showId: ExternalId;
  row: RowEpisode[];
}) {
  return (
    <TitleTop
      title={show}
      kind="Show"
      id={showId}
      facts={[show.year?.toString()]}
      actions={
        <>
          <SeasonAsk target={heroTarget(row)} />
          <SeasonAskRefusal />
        </>
      }
      more={<ShowRequestMenu row={row} />}
    />
  );
}
