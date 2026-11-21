import { notFound } from "@tanstack/react-router";

import { Overview } from "@/features/title/components/overview";
import { SeasonPicker } from "@/features/title/components/season-picker";
import { TitleDetails } from "@/features/title/components/title-details";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitleGenres } from "@/features/title/components/title-genres";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import { TitlePage } from "@/features/title/components/title-page";
import { useFoundShow } from "@/features/title/data/use-found-show";
import { originalAudio } from "@/features/title/lib/original-audio";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Show from the title source, for the signed-in User `you` to pick its Seasons to Request: its
 * top, then its Seasons where a Show on the Server has its Episodes.
 */
export function ShowToRequest({ showId, you }: { showId: ExternalId; you: bigint | undefined }) {
  const show = useFoundShow({ showId });
  if (!show) {
    throw notFound();
  }
  return (
    <TitlePage
      hero={
        <TitleHero backdrop={{ url: show.backdropUrl, thumbhash: show.backdropThumbhash }}>
          <TitleName title={show.title} logoUrl={undefined} />
          <TitleGenres kind="Show" genres={[]} />
          {show.overview !== undefined && <Overview text={show.overview} />}
          <TitleFacts score={show.score} facts={[show.year?.toString()]} boxed={[]} />
        </TitleHero>
      }
    >
      <SeasonPicker show={show} you={you} />
      <TitleDetails
        groups={[{ heading: "Languages", facts: [originalAudio(show.originalLanguage)] }]}
      />
    </TitlePage>
  );
}
