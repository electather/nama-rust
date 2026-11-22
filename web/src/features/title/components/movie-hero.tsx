import { useState } from "react";

import { MovieMoreLoader } from "@/features/title/components/movie-more-loader";
import { Play } from "@/features/title/components/play";
import { TitleTop } from "@/features/title/components/title-top";
import { titleBadges } from "@/features/title/lib/title-badges";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
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
    <TitleTop
      title={movie}
      kind="Movie"
      id={movieId}
      logoUrl={movie.titleLogoUrl}
      facts={[yearOf(movie.releaseDate), runtimeOf(movie.runtimeMinutes, "short")]}
      boxed={titleBadges(movie.versions)}
      actions={<Play params={params} version={version} watchState={movie.watchState} />}
      more={
        <MovieMoreLoader
          {...{ libraryId, movieId, movie, readOnlyFolder, version }}
          onChoose={setChosen}
        />
      }
    />
  );
}
