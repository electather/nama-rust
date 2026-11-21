import { Overview } from "@/features/title/components/overview";
import { TitleActions } from "@/features/title/components/title-actions";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitleGenres } from "@/features/title/components/title-genres";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { FoundShow } from "@/gen/nama/v1/request_pb";
import { WatchlistButton } from "@/shared/components/watchlist-button";

/**
 * The top of a Show's page that isn't on the Server: its title over its Backdrop, its overview and
 * facts, then the Watchlist's button for the Show `showId`.
 */
export function ShowRequestHero({ showId, show }: { showId: ExternalId; show: FoundShow }) {
  return (
    <TitleHero backdrop={{ url: show.backdropUrl, thumbhash: show.backdropThumbhash }}>
      <TitleName title={show.title} logoUrl={undefined} />
      <TitleGenres kind="Show" genres={show.genres} />
      {show.overview !== undefined && <Overview text={show.overview} />}
      <TitleFacts score={show.score} facts={[show.year?.toString()]} boxed={[show.contentRating]} />
      <TitleActions>
        <WatchlistButton watchlist={{ kind: TitleKind.SHOW, id: showId, on: show.onWatchlist }} />
      </TitleActions>
    </TitleHero>
  );
}
