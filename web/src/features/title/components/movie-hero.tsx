import { useState } from "react";

import { MovieMoreLoader } from "@/features/title/components/movie-more-loader";
import { Overview } from "@/features/title/components/overview";
import { Play } from "@/features/title/components/play";
import { TitleActions } from "@/features/title/components/title-actions";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitleGenres } from "@/features/title/components/title-genres";
import { TitleHero } from "@/features/title/components/title-hero";
import { TitleName } from "@/features/title/components/title-name";
import { titleBadges } from "@/features/title/lib/title-badges";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import { WatchlistButton } from "@/shared/components/watchlist-button";
import { runtimeOf, yearOf } from "@/shared/lib/title-facts";
import { versionToPlay } from "@/shared/lib/version-to-play";

/**
 * The top of a Movie's page, which says what it is and plays it. `params` are the page's URL, which
 * Play links from.
 */
export function MovieHero({
  libraryId,
  movieId,
  movie,
  readOnlyFolder,
  params,
}: {
  libraryId: bigint;
  movieId: ExternalId;
  movie: Movie;
  readOnlyFolder: string | undefined;
  params: { libraryId: string; movieId: string };
}) {
  const [chosen, setChosen] = useState<number>();
  // The largest until one is chosen, and again if an Event removed the chosen one.
  const version = versionToPlay(movie.versions, chosen);
  return (
    <TitleHero backdrop={{ url: movie.backdropUrl, thumbhash: movie.backdropThumbhash }}>
      <TitleName title={movie.title} logoUrl={movie.titleLogoUrl} />
      <TitleGenres kind="Movie" genres={movie.genres} />
      {movie.overview !== undefined && <Overview text={movie.overview} />}
      <TitleFacts
        facts={[yearOf(movie.releaseDate), runtimeOf(movie.runtimeMinutes, "short")]}
        boxed={[movie.contentRating, ...titleBadges(movie.versions)]}
      />
      <TitleActions>
        <Play params={params} version={version} watchState={movie.watchState} />
        <WatchlistButton
          watchlist={{ kind: TitleKind.MOVIE, id: movieId, on: movie.onWatchlist }}
        />
        <MovieMoreLoader
          {...{ libraryId, movieId, movie, readOnlyFolder, version }}
          onChoose={setChosen}
        />
      </TitleActions>
    </TitleHero>
  );
}
