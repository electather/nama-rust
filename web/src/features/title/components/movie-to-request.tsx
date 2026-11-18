import { notFound } from "@tanstack/react-router";

import { TitlePage } from "@/features/title/components/title-page";
import { useFoundMovie } from "@/features/title/data/use-found-movie";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { FoundMovieState } from "@/shared/components/found-movie-state/found-movie-state";
import { metadataLine, runtimeOf } from "@/shared/lib/title-facts";

/** A Movie from the title source, for the signed-in User `you` to Request. */
export function MovieToRequest({ movieId, you }: { movieId: ExternalId; you: bigint | undefined }) {
  const movie = useFoundMovie({ movieId });
  if (!movie) {
    throw notFound();
  }
  return (
    <TitlePage
      title={movie.title}
      backdropUrl={movie.backdropUrl}
      backdropThumbhash={movie.backdropThumbhash}
    >
      <p className="text-sm text-muted-foreground">
        {metadataLine(movie.year?.toString(), runtimeOf(movie.runtimeMinutes))}
      </p>
      <div className="mt-6 flex flex-wrap items-center gap-3">
        <FoundMovieState movie={movie} you={you} primary />
      </div>
      {movie.overview !== undefined && (
        <p className="mt-8 max-w-prose text-pretty">{movie.overview}</p>
      )}
    </TitlePage>
  );
}
