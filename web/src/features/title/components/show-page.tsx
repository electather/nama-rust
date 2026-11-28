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
import { useFoundShowIfMay } from "@/features/title/data/use-found-show-if-may";
import { useShow } from "@/features/title/data/use-show";
import { useShowEpisodes } from "@/features/title/data/use-show-episodes";
import { SeasonAskingContext, usePageSeasonAsking } from "@/features/title/hooks/season-asking";
import { useSeasonRow } from "@/features/title/hooks/use-season-row";
import { episodeToResume } from "@/features/title/lib/episode-to-resume";
import { heroTarget, targetSeason } from "@/features/title/lib/hero-target";
import { openingSeason, seasonShown } from "@/features/title/lib/opening-season";
import { rowSeasons } from "@/features/title/lib/row-seasons";
import { seasonsOfRow, showRow } from "@/features/title/lib/show-row";
import { similarShows } from "@/features/title/lib/similar-titles";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { isGone } from "@/shared/hooks/browse";
import { useMay } from "@/shared/hooks/use-may";
import type { DeletedSearch } from "@/shared/lib/deleted-search";
import { backdropOf } from "@/shared/lib/server-image";

/**
 * A Show of a Library: its top, with Play or Resume, or Request for a Season it lacks, and what else
 * may be done, every Episode in one row under the name of the Season it's on, a menu that scrolls
 * it to another, its Trailers, then its Details. To a User who may Request Shows, the row holds
 * the Episodes missing from the Server too, dimmed. The row opens on the Season `asked`, if it
 * has it, or else on the Season of the main button's Episode. Then the Similar titles in a Library.
 * Over the row, what a delete on the page left to `report`.
 */
export function ShowPage({
  libraryId,
  showId,
  asked,
  report,
}: {
  libraryId: bigint;
  showId: ExternalId;
  asked: number | undefined;
  report: DeletedSearch;
}) {
  const show = { libraryId, showId };
  const { data, error } = useShow(show);
  const librarySeasons = rowSeasons(data.seasons);
  const episodes = useShowEpisodes(show, librarySeasons);
  const found = useFoundShowIfMay(showId);
  const asking = usePageSeasonAsking(found);
  const row = showRow(episodes, found, asking?.you);
  const seasons = seasonsOfRow(row, librarySeasons);
  const resumed = episodeToResume(useContinueWatchingEntries(), libraryId, showId);
  const target = heroTarget(row, resumed, useMay(Permission.MANAGE_REQUESTS));
  const toPlay = targetSeason(target);
  const {
    row: rowRef,
    scroller,
    season: rowSeason,
    opening,
    pick,
  } = useSeasonRow(openingSeason(seasons, asked, toPlay), row);
  const season = seasonShown(seasons, rowSeason, toPlay);
  if (!data.show || isGone(error)) {
    throw notFound();
  }
  return (
    <SeasonAskingContext value={asking}>
      <TitlePage
        name={data.show.title}
        hero={
          <ShowHero
            {...{ libraryId, showId, episodes, row, target }}
            show={data.show}
            seasons={data.seasons}
            seasonNumber={season}
          />
        }
      >
        <NotToldNotice {...report} />
        <ShowEpisodes
          {...{ show, seasons, scroller, opening }}
          row={rowRef}
          episodes={row}
          current={season}
          onPick={pick}
          backdrop={backdropOf(data.show)}
        />
        <TitleTrailers trailers={data.show.trailers} art={backdropOf(data.show)} />
        <SimilarTitles heading="Similar titles" titles={similarShows(data.similar)} />
        <CastAndCrew cast={data.show.cast} crew={data.show.crew} />
        <ShowDetails
          libraryId={libraryId}
          show={data.show}
          episodes={episodes}
          aired={data.airedEpisodeCount}
        />
      </TitlePage>
    </SeasonAskingContext>
  );
}
