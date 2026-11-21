import { notFound } from "@tanstack/react-router";

import { CastAndCrew } from "@/features/title/components/cast-and-crew";
import { NotToldNotice } from "@/features/title/components/not-told-notice";
import { ShowDetails } from "@/features/title/components/show-details";
import { ShowEpisodes } from "@/features/title/components/show-episodes";
import { ShowHero } from "@/features/title/components/show-hero";
import { SimilarTitles } from "@/features/title/components/similar-titles";
import { TitlePage } from "@/features/title/components/title-page";
import { TitleTrailers } from "@/features/title/components/title-trailers";
import { useContinueWatchingEntries } from "@/features/title/data/use-continue-watching-entries";
import { useShow } from "@/features/title/data/use-show";
import { useShowEpisodes } from "@/features/title/data/use-show-episodes";
import { useSeasonRow } from "@/features/title/hooks/use-season-row";
import { episodeToResume } from "@/features/title/lib/episode-to-resume";
import { nextToWatch } from "@/features/title/lib/next-to-watch";
import { openingSeason, seasonShown } from "@/features/title/lib/opening-season";
import { rowSeasons } from "@/features/title/lib/row-seasons";
import { similarShows } from "@/features/title/lib/similar-titles";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { isGone } from "@/shared/hooks/browse";

/**
 * A Show of a Library: its top, with Play or Resume and what else may be done, every Episode in one
 * row under the name of the Season it's on, a menu that scrolls it to another, its Trailers, then
 * its Details.
 * The row opens on the Season `asked`, if it has it, or else on the Season of the Episode Play
 * starts. Then the Similar titles in a Library.
 */
export function ShowPage({
  libraryId,
  showId,
  asked,
}: {
  libraryId: bigint;
  showId: ExternalId;
  asked: number | undefined;
}) {
  const show = { libraryId, showId };
  const { data, error } = useShow(show);
  const seasons = rowSeasons(data.seasons);
  const episodes = useShowEpisodes(show, seasons);
  const entries = useContinueWatchingEntries();
  const toPlay = episodeToResume(entries, libraryId, showId) ?? nextToWatch(episodes);
  const {
    row,
    scroller,
    season: rowSeason,
    opening,
    pick,
  } = useSeasonRow(openingSeason(seasons, asked, toPlay), episodes);
  const season = seasonShown(seasons, rowSeason, toPlay);
  if (!data.show || isGone(error)) {
    throw notFound();
  }
  return (
    <TitlePage
      hero={
        <ShowHero
          {...{ libraryId, showId, episodes, toPlay }}
          show={data.show}
          seasons={data.seasons}
          seasonNumber={season}
        />
      }
    >
      <NotToldNotice />
      <ShowEpisodes
        {...{ libraryId, seasons, episodes, row, scroller, opening }}
        current={season}
        onPick={pick}
      />
      <TitleTrailers
        trailers={data.show.trailers}
        art={{ url: data.show.backdropUrl, thumbhash: data.show.backdropThumbhash }}
      />
      <SimilarTitles heading="Similar titles" titles={similarShows(data.similar)} />
      <CastAndCrew cast={data.show.cast} crew={data.show.crew} />
      <ShowDetails
        libraryId={libraryId}
        show={data.show}
        episodes={episodes}
        aired={data.airedEpisodeCount}
      />
    </TitlePage>
  );
}
