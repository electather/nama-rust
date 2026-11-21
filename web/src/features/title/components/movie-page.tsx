import { notFound } from "@tanstack/react-router";

import { MovieDetails } from "@/features/title/components/movie-details";
import { MovieHero } from "@/features/title/components/movie-hero";
import { TitlePage } from "@/features/title/components/title-page";
import { TitleTrailers } from "@/features/title/components/title-trailers";
import { useMovie } from "@/features/title/data/use-movie";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Movie of a Library: its top, with Play and what else may be done, its Trailers, then its
 * Details.
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
  const { movie, readOnlyFolder } = useMovie({ libraryId, movieId });
  if (!movie) {
    throw notFound();
  }
  return (
    <TitlePage hero={<MovieHero {...{ libraryId, movieId, movie, readOnlyFolder, params }} />}>
      <TitleTrailers trailers={movie.trailers} />
      <MovieDetails libraryId={libraryId} movie={movie} />
    </TitlePage>
  );
}
