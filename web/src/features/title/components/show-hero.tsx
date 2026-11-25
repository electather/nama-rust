import { ShowMoreLoader } from "@/features/title/components/show-more-loader";
import { ShowPlay } from "@/features/title/components/show-play";
import { TitleTop } from "@/features/title/components/title-top";
import { useEpisodeElsewhere } from "@/features/title/data/use-episode-elsewhere";
import { usePlayFrom } from "@/features/title/hooks/use-play-from";
import { seasonsOf } from "@/features/title/lib/seasons-of";
import { titleBadges } from "@/features/title/lib/title-badges";
import type { Episode, Season, Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { yearOf } from "@/shared/lib/title-facts";

/**
 * The top of a Show's page, which says what it is and plays `toPlay`, at the Hub from whichever of
 * the Account's Servers has it best. Its "…" menu acts on the Show, and edits the Poster of the
 * Season the row is on, `seasonNumber`, with the Show's images.
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
  const playFrom = usePlayFrom(
    toPlay && { libraryId, versions: toPlay.versions, watchState: toPlay.watchState },
    useEpisodeElsewhere(showId, toPlay),
  );
  return (
    <TitleTop
      title={show}
      kind="Show"
      id={showId}
      logoUrl={show.titleLogoUrl}
      facts={[yearOf(show.firstAirDate), seasonsOf(show.seasonCount)]}
      boxed={titleBadges(episodes.flatMap(({ versions }) => versions))}
      actions={
        toPlay &&
        playFrom.choice && <ShowPlay showId={showId} episode={toPlay} choice={playFrom.choice} />
      }
      more={
        <ShowMoreLoader
          {...{ libraryId, showId, show, playFrom }}
          season={seasons.find(({ number }) => number === seasonNumber)}
        />
      }
    />
  );
}
