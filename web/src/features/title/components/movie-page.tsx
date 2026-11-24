import { notFound } from "@tanstack/react-router";

import { BoxSetRow } from "@/features/title/components/box-set-row";
import { CastAndCrew } from "@/features/title/components/cast-and-crew";
import { MovieDetails } from "@/features/title/components/movie-details";
import { MovieHero } from "@/features/title/components/movie-hero";
import { SimilarTitles } from "@/features/title/components/similar-titles";
import { TitlePage } from "@/features/title/components/title-page";
import { TitleTrailers } from "@/features/title/components/title-trailers";
import { useMovie } from "@/features/title/data/use-movie";
import { similarMovies } from "@/features/title/lib/similar-titles";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Movie of a Library: its top, with Play and what else may be done, its Trailers, its Box Set,
 * the Similar titles in a Library, then its Details.
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
  const { movie, readOnlyFolder, similar, boxSet } = useMovie({ libraryId, movieId });
  if (!movie) {
    throw notFound();
  }
  return (
    <TitlePage
      name={movie.title}
      hero={<MovieHero {...{ libraryId, movieId, movie, readOnlyFolder, params }} />}
    >
      <TitleTrailers
        trailers={movie.trailers}
        art={{ url: movie.backdropUrl, thumbhash: movie.backdropThumbhash }}
      />
      <BoxSetRow boxSet={boxSet} current={movie.id} />
      <SimilarTitles heading="Similar titles" titles={similarMovies(similar)} />
      <CastAndCrew cast={movie.cast} crew={movie.crew} />
      <MovieDetails libraryId={libraryId} movie={movie} />
    </TitlePage>
  );
}
