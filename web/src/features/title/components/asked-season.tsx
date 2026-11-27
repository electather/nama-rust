import { SeasonRequestButton } from "@/features/title/components/season-request-button";
import type { SeasonAsking } from "@/features/title/hooks/season-asking";
import { RequestStateAndQuality } from "@/shared/components/found-movie-state/request-state-and-quality";
import type { Standing } from "@/shared/lib/request-standing";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * Where `season` stands with its Request, by its `standing`, and "Request Season N" while you can
 * ask, or join someone else's.
 */
export function AskedSeason({
  asking,
  season,
  standing,
}: {
  asking: SeasonAsking;
  season: number;
  standing: Standing;
}) {
  const what = `${seasonName(season)} of ${asking.show.title}`;
  return (
    <>
      {"request" in standing && <RequestStateAndQuality standing={standing} what={what} large />}
      <SeasonRequestButton asking={asking} season={season} standing={standing} />
    </>
  );
}
