import { Overview } from "@/features/title/components/overview";
import { ShowMoreLoader } from "@/features/title/components/show-more-loader";
import { ShowPlay } from "@/features/title/components/show-play";
import { TitleActions } from "@/features/title/components/title-actions";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitleGenres } from "@/features/title/components/title-genres";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import { seasonsOf } from "@/features/title/lib/seasons-of";
import { titleBadges } from "@/features/title/lib/title-badges";
import type { Episode, Season, Show } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import { WatchlistButton } from "@/shared/components/watchlist-button";
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
    <TitleHero backdrop={{ url: show.backdropUrl, thumbhash: show.backdropThumbhash }}>
      <TitleName title={show.title} logoUrl={show.titleLogoUrl} />
      <TitleGenres kind="Show" genres={show.genres} />
      {show.overview !== undefined && <Overview text={show.overview} />}
      <TitleFacts
        facts={[yearOf(show.firstAirDate), seasonsOf(show.seasonCount)]}
        boxed={[show.contentRating, ...titleBadges(episodes.flatMap(({ versions }) => versions))]}
      />
      <TitleActions>
        {toPlay && <ShowPlay libraryId={libraryId} showId={showId} episode={toPlay} />}
        <WatchlistButton watchlist={{ kind: TitleKind.SHOW, id: showId, on: show.onWatchlist }} />
        <ShowMoreLoader
          {...{ libraryId, showId, show }}
          season={seasons.find(({ number }) => number === seasonNumber)}
        />
      </TitleActions>
    </TitleHero>
  );
}
