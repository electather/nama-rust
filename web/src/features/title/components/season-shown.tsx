import { Episodes } from "@/features/title/components/episodes";
import { Specials } from "@/features/title/components/specials";
import { specialsBelow } from "@/features/title/lib/season-to-show";
import type { Season } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * The Season shown's Episodes, and any Specials below a Show's only Season, or that it's gone,
 * once an Event says it was deleted.
 */
export function SeasonShown({
  libraryId,
  showId,
  seasons,
  current,
}: {
  libraryId: bigint;
  showId: ExternalId;
  seasons: Season[];
  current: number;
}) {
  if (seasons.some(({ number }) => number === current)) {
    return (
      <>
        <section aria-label={`${seasonName(current)} Episodes`}>
          <Episodes libraryId={libraryId} showId={showId} seasonNumber={current} />
        </section>
        {specialsBelow(seasons, current) && <Specials libraryId={libraryId} showId={showId} />}
      </>
    );
  }
  return (
    <p className="mt-6 text-muted-foreground">
      {seasonName(current)} is no longer in this Library.
    </p>
  );
}
