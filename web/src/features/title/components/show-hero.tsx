import { ShowMoreLoader } from "@/features/title/components/show-more-loader";
import { ShowPlay } from "@/features/title/components/show-play";
import { TitleTop } from "@/features/title/components/title-top";
import { seasonsOf } from "@/features/title/lib/seasons-of";
import { titleBadges } from "@/features/title/lib/title-badges";
import type { Episode, Season, Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { yearOf } from "@/shared/lib/title-facts";

/**
 * The top of a Show's page, which says what it is and plays `toPlay`. Its "…" menu acts on the
 * Show, and edits the Poster of the Season the row is on, `seasonNumber`, with the Show's images.
 */
export function ShowHero({
  libraryId,
  showId,
  show,
  seasons,
  episodes,
  toPlay,
  seasonNumber,
}: {
  libraryId: bigint;
  showId: ExternalId;
  show: Show;
  seasons: Season[];
  episodes: Episode[];
  toPlay: Episode | undefined;
  seasonNumber: number;
}) {
  return (
    <TitleTop
      title={show}
      kind="Show"
      id={showId}
      logoUrl={show.titleLogoUrl}
      facts={[yearOf(show.firstAirDate), seasonsOf(show.seasonCount)]}
      boxed={titleBadges(episodes.flatMap(({ versions }) => versions))}
      actions={toPlay && <ShowPlay libraryId={libraryId} showId={showId} episode={toPlay} />}
      more={
        <ShowMoreLoader
          {...{ libraryId, showId, show }}
          season={seasons.find(({ number }) => number === seasonNumber)}
        />
      }
    />
  );
}
