import { notFound } from "@tanstack/react-router";

import { CastAndCrew } from "@/features/title/components/cast-and-crew";
import { PeopleLinks } from "@/features/title/components/people-links";
import { SeasonPicker } from "@/features/title/components/season-picker";
import { SimilarTitles } from "@/features/title/components/similar-titles";
import { TitleDetails } from "@/features/title/components/title-details";
import { TitlePage } from "@/features/title/components/title-page";
import { TitleTop } from "@/features/title/components/title-top";
import { TitleTrailers } from "@/features/title/components/title-trailers";
import { useFoundShow } from "@/features/title/data/use-found-show";
import { originalAudio } from "@/features/title/lib/original-audio";
import { crewWithJob } from "@/features/title/lib/people";
import { similarShows } from "@/features/title/lib/similar-titles";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Show from the title source, for the signed-in User `you` to pick its Seasons to Request: its
 * top, its Seasons where a Show on the Server has its Episodes, its Trailers, the Similar titles
 * on this Server, then its Details.
 */
export function ShowToRequest({ showId, you }: { showId: ExternalId; you: bigint | undefined }) {
  const show = useFoundShow({ showId });
  if (!show) {
    throw notFound();
  }
  const creators = crewWithJob(show.crew, "Creator");
  return (
    <TitlePage
      hero={<TitleTop title={show} kind="Show" id={showId} facts={[show.year?.toString()]} />}
    >
      <SeasonPicker show={show} you={you} />
      <TitleTrailers
        trailers={show.trailers}
        art={{ url: show.backdropUrl, thumbhash: show.backdropThumbhash }}
      />
      <SimilarTitles heading="On this Server" titles={similarShows(show.similar)} />
      <CastAndCrew cast={show.cast} crew={show.crew} />
      <TitleDetails
        groups={[
          {
            heading: "Information",
            facts: [["Created by", creators && <PeopleLinks key="crew" people={creators} />]],
          },
          { heading: "Languages", facts: [originalAudio(show.originalLanguage)] },
        ]}
      />
    </TitlePage>
  );
}
