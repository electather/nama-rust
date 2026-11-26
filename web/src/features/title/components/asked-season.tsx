import { SeasonRequestButton } from "@/features/title/components/season-request-button";
import type { SeasonAsking } from "@/features/title/hooks/season-asking";
import type { Request } from "@/gen/nama/v1/request_pb";
import { RequestStateAndQuality } from "@/shared/components/found-movie-state/request-state-and-quality";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * Where `season`'s `request` is, if it has one, and "Request Season N" while you can ask, or join
 * someone else's.
 */
export function AskedSeason({
  asking,
  season,
  request,
}: {
  asking: SeasonAsking;
  season: number;
  request: Request | undefined;
}) {
  const what = `${seasonName(season)} of ${asking.show.title}`;
  return (
    <>
      {request && <RequestStateAndQuality request={request} what={what} large />}
      <SeasonRequestButton asking={asking} season={season} request={request} />
    </>
  );
}
