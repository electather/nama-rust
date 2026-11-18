import { notFound } from "@tanstack/react-router";

import { DeleteFromShow } from "@/features/title/components/delete-from-show";
import { EditImages } from "@/features/title/components/edit-images";
import { MarkPlayedMenu } from "@/features/title/components/mark-played-menu";
import { SeasonShown } from "@/features/title/components/season-shown";
import { Seasons } from "@/features/title/components/seasons";
import { ShowIntro } from "@/features/title/components/show-intro";
import { TitlePage } from "@/features/title/components/title-page";
import { useEpisodes } from "@/features/title/data/use-episodes";
import { useShow } from "@/features/title/data/use-show";
import { showImages } from "@/features/title/lib/title-images";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { isGone } from "@/lib/browse";

/**
 * A Show of a Library: its Title Logo over its backdrop, its intro, the Season shown with its
 * Episodes, and what may be done to the Show.
 */
export function ShowPage({
  libraryId,
  showId,
  seasonNumber,
}: {
  libraryId: bigint;
  showId: ExternalId;
  seasonNumber: number;
}) {
  const show = { libraryId, showId };
  const { data, error } = useShow(show);
  const season = useEpisodes({ ...show, seasonNumber });
  if (!data.show || isGone(error)) {
    throw notFound();
  }
  return (
    <TitlePage
      title={data.show.title}
      logoUrl={data.show.titleLogoUrl}
      backdropUrl={data.show.backdropUrl}
      backdropThumbhash={data.show.backdropThumbhash}
    >
      <ShowIntro {...show} show={data.show} />
      {/* Ends with the menu even when there are no Season links before it. */}
      <div className="mt-8 flex items-start justify-end gap-2">
        <Seasons seasons={data.seasons} current={seasonNumber} />
        <MarkPlayedMenu
          {...show}
          seasonNumber={seasonNumber}
          seasonPlayed={
            season.episodes.length > 0 &&
            season.episodes.every(({ watchState }) => watchState?.played === true)
          }
        />
      </div>
      <SeasonShown {...show} seasons={data.seasons} current={seasonNumber} />
      {/* Out of the way below the Episodes, and pulled out by the buttons' own padding. */}
      <div className="-ms-4 mt-6 flex max-w-3xl flex-wrap gap-2 empty:hidden">
        <EditImages
          title={data.show.title}
          {...showImages(
            showId,
            data.show,
            data.seasons.find(({ number }) => number === seasonNumber),
          )}
        />
        <DeleteFromShow
          {...show}
          title={data.show.title}
          seasons={data.seasons}
          current={seasonNumber}
        />
      </div>
    </TitlePage>
  );
}
