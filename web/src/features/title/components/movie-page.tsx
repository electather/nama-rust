import { notFound } from "@tanstack/react-router";
import { useState } from "react";

import { MovieActions } from "@/features/title/components/movie-actions";
import { Overview } from "@/features/title/components/overview";
import { Play } from "@/features/title/components/play";
import { TitleFacts } from "@/features/title/components/title-facts";
import { TitlePage } from "@/features/title/components/title-page";
import { useMovie } from "@/features/title/data/use-movie";
import { movieBadges } from "@/features/title/lib/movie-badges";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { versionToPlay } from "@/lib/browse";
import { runtimeOf, yearOf } from "@/shared/lib/title-facts";

/**
 * A Movie of a Library: its Title Logo over its backdrop, its facts, Play or Resume with a choice
 * of Version, its actions and its overview. `params` are the page's URL, which Play links from.
 */
export function MoviePage({
  libraryId,
  movieId,
  params,
}: {
  libraryId: bigint;
  movieId: ExternalId;
  params: { libraryId: string; movieId: string };
}) {
  const data = useMovie({ libraryId, movieId });
  const [chosen, setChosen] = useState<number>();
  const { movie } = data;
  if (!movie) {
    throw notFound();
  }
  // The largest until one is chosen, and again if an Event removed the chosen one.
  const version = versionToPlay(movie.versions, chosen);
  return (
    <TitlePage
      title={movie.title}
      logoUrl={movie.titleLogoUrl}
      backdropUrl={movie.backdropUrl}
      backdropThumbhash={movie.backdropThumbhash}
    >
      <TitleFacts
        facts={[
          "Movie",
          ...movie.genres,
          yearOf(movie.releaseDate),
          runtimeOf(movie.runtimeMinutes, "short"),
        ]}
        boxed={[movie.contentRating, ...movieBadges(movie.versions)]}
      />
      <Play
        params={params}
        versions={movie.versions}
        version={version}
        watchState={movie.watchState}
        onChoose={setChosen}
      >
        <MovieActions
          libraryId={libraryId}
          movieId={movieId}
          movie={movie}
          readOnlyFolder={data.readOnlyFolder}
        />
      </Play>
      {movie.overview !== undefined && <Overview text={movie.overview} />}
    </TitlePage>
  );
}
