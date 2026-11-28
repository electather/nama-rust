import { notFound } from "@tanstack/react-router";

import { CastAndCrew } from "@/features/title/components/cast-and-crew";
import { FoundShowDetails } from "@/features/title/components/found-show-details";
import { FoundShowHero } from "@/features/title/components/found-show-hero";
import { ShowEpisodes } from "@/features/title/components/show-episodes";
import { SimilarTitles } from "@/features/title/components/similar-titles";
import { TitlePage } from "@/features/title/components/title-page";
import { TitleTrailers } from "@/features/title/components/title-trailers";
import { useFoundShow } from "@/features/title/data/use-found-show";
import { SeasonAskingContext, usePageSeasonAsking } from "@/features/title/hooks/season-asking";
import { useShowRow } from "@/features/title/hooks/use-show-row";
import { similarShows } from "@/features/title/lib/similar-titles";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { backdropOf } from "@/shared/lib/server-image";

/**
 * A Show from the title source, to Request: its top, with "Request Season N" for the first Season
 * missing from the Server, or where its Request is, and a "…" that Requests many; every Episode in
 * one row, those missing dimmed; its Trailers, the Similar titles on this Server, then its Details.
 */
export function ShowToRequest({ showId }: { showId: ExternalId }) {
  const show = useFoundShow({ showId });
  const asking = usePageSeasonAsking(show);
  const row = useShowRow({ asking });
  if (!show) {
    throw notFound();
  }
  return (
    <SeasonAskingContext value={asking}>
      <TitlePage name={show.title} hero={<FoundShowHero {...{ show, showId, row }} />}>
        <ShowEpisodes row={row} show={undefined} backdrop={backdropOf(show)} />
        <TitleTrailers trailers={show.trailers} art={backdropOf(show)} />
        <SimilarTitles heading="On this Server" titles={similarShows(show.similar)} />
        <CastAndCrew cast={show.cast} crew={show.crew} />
        <FoundShowDetails show={show} />
      </TitlePage>
    </SeasonAskingContext>
  );
}
