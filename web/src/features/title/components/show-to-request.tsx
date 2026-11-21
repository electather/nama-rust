import { notFound } from "@tanstack/react-router";

import { SeasonPicker } from "@/features/title/components/season-picker";
import { ShowRequestHero } from "@/features/title/components/show-request-hero";
import { SimilarTitles } from "@/features/title/components/similar-titles";
import { TitleDetails } from "@/features/title/components/title-details";
import { TitlePage } from "@/features/title/components/title-page";
import { TitleTrailers } from "@/features/title/components/title-trailers";
import { useFoundShow } from "@/features/title/data/use-found-show";
import { originalAudio } from "@/features/title/lib/original-audio";
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
  return (
    <TitlePage hero={<ShowRequestHero showId={showId} show={show} />}>
      <SeasonPicker show={show} you={you} />
      <TitleTrailers trailers={show.trailers} />
      <SimilarTitles heading="On this Server" titles={similarShows(show.similar)} />
      <TitleDetails
        groups={[{ heading: "Languages", facts: [originalAudio(show.originalLanguage)] }]}
      />
    </TitlePage>
  );
}
