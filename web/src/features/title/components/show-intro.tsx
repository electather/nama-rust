import { Overview } from "@/features/title/components/overview";
import { Resume } from "@/features/title/components/resume";
import { TitleFacts } from "@/features/title/components/title-facts";
import type { Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { yearOf } from "@/shared/lib/title-facts";

/**
 * The top of a Show's page under its title: its genres, first air year and Content Rating, Resume
 * for its Episode in your Continue Watching if there is one, and its own overview.
 */
export function ShowIntro({
  libraryId,
  showId,
  show,
}: {
  libraryId: bigint;
  showId: ExternalId;
  show: Show;
}) {
  return (
    <>
      <TitleFacts
        facts={["Show", ...show.genres, yearOf(show.firstAirDate)]}
        boxed={[show.contentRating]}
      />
      <Resume libraryId={libraryId} showId={showId} />
      {show.overview !== undefined && <Overview text={show.overview} />}
    </>
  );
}
