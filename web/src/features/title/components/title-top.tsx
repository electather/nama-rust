import type { ReactNode } from "react";

import { Overview } from "@/features/title/components/overview";
import { TitleActions } from "@/features/title/components/title-actions";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitleGenres } from "@/features/title/components/title-genres";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import { TitleStarring } from "@/features/title/components/title-starring";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import { WatchlistButton } from "@/shared/components/watchlist-button";
import { type ServerImage, backdropOf } from "@/shared/lib/server-image";

const watchlistKind = { Movie: TitleKind.MOVIE, Show: TitleKind.SHOW };
const noBoxed: (string | undefined)[] = [];

/**
 * The top every title's page shares: its name, or `logo`, the Title Logo only a title in a
 * Library has, genres, overview, starring line, facts with `boxed` after the Content Rating, then
 * `actions`, the Watchlist button and `more`. `title` is a Movie or Show, in a Library or from the
 * title source, whose four messages share these fields; `id` is only what the Watchlist button
 * needs.
 */
export function TitleTop({
  title,
  kind,
  id,
  logo,
  facts,
  boxed = noBoxed,
  actions,
  more,
}: {
  title: Pick<
    Movie,
    | "title"
    | "genres"
    | "overview"
    | "cast"
    | "score"
    | "contentRating"
    | "onWatchlist"
    | "backdropUrl"
    | "backdropThumbhash"
  >;
  kind: "Movie" | "Show";
  id: ExternalId;
  logo?: ServerImage | undefined;
  facts: (string | undefined)[];
  boxed?: (string | undefined)[];
  actions?: ReactNode;
  more?: ReactNode;
}) {
  return (
    <TitleHero backdrop={backdropOf(title)}>
      <TitleName title={title.title} logo={logo} />
      <TitleGenres kind={kind} genres={title.genres} />
      {title.overview !== undefined && <Overview text={title.overview} />}
      <TitleStarring cast={title.cast} />
      <TitleFacts score={title.score} facts={facts} boxed={[title.contentRating, ...boxed]} />
      <TitleActions>
        {actions}
        <WatchlistButton watchlist={{ kind: watchlistKind[kind], id, on: title.onWatchlist }} />
        {more}
      </TitleActions>
    </TitleHero>
  );
}
