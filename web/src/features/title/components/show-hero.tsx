import { SeasonAsk } from "@/features/title/components/season-ask";
import { SeasonAskRefusal } from "@/features/title/components/season-ask-refusal";
import { ShowMoreLoader } from "@/features/title/components/show-more-loader";
import { ShowPlay } from "@/features/title/components/show-play";
import { TitleTop } from "@/features/title/components/title-top";
import { useEpisodeElsewhere } from "@/features/title/data/use-episode-elsewhere";
import { usePlayFrom } from "@/features/title/hooks/use-play-from";
import type { ShowRow } from "@/features/title/hooks/use-show-row";
import { targetEpisode } from "@/features/title/lib/hero-target";
import { seasonsOf } from "@/features/title/lib/seasons-of";
import { titleBadges } from "@/features/title/lib/title-badges";
import type { Episode, Season, Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { titleLogoOf } from "@/shared/lib/server-image";
import { yearOf } from "@/shared/lib/title-facts";

/**
 * The top of a Show's page, which says what it is and does its `row`'s main button's target, the
 * next thing you'd watch: plays its Episode, at the Hub from whichever of the Account's Servers has
 * it best, or Requests its Season. Its "…" menu acts on the Show, Requests the Seasons of the row
 * it lacks, and edits the Poster of the Season the row is on, with the Show's images.
 */
export function ShowHero({
  libraryId,
  showId,
  show,
  seasons,
  episodes,
  row,
}: {
  libraryId: bigint;
  showId: ExternalId;
  show: Show;
  seasons: Season[];
  episodes: Episode[];
  row: ShowRow;
}) {
  const toPlay = targetEpisode(row.target);
  const playFrom = usePlayFrom(
    toPlay && { libraryId, versions: toPlay.versions, watchState: toPlay.watchState },
    useEpisodeElsewhere(showId, toPlay),
  );
  return (
    <TitleTop
      title={show}
      kind="Show"
      id={showId}
      logo={titleLogoOf(show)}
      facts={[yearOf(show.firstAirDate), seasonsOf(show.seasonCount)]}
      boxed={titleBadges(episodes.flatMap(({ versions }) => versions))}
      actions={
        <>
          {toPlay && playFrom.choice && (
            <ShowPlay showId={showId} episode={toPlay} choice={playFrom.choice} />
          )}
          <SeasonAsk target={row.target} />
          <SeasonAskRefusal />
        </>
      }
      more={
        <ShowMoreLoader
          {...{ libraryId, showId, show, playFrom }}
          row={row.episodes}
          season={seasons.find(({ number }) => number === row.season)}
        />
      }
    />
  );
}
