import { Link } from "@tanstack/react-router";

import { FoundMovieState } from "@/components/found-movie-state";
import type { MovieHit } from "@/gen/nama/v1/request_pb";
import { Poster } from "@/shared/components/poster/poster";
import { posterLink } from "@/shared/lib/poster-classes";
import { titleParam } from "@/shared/lib/title-param";

/** A Movie found: its poster opens its page, in its Library if one has it. */
export function MovieCard({ movie, you }: { movie: MovieHit; you: bigint | undefined }) {
  const movieId = titleParam(movie.id);
  const poster = (
    <Poster
      url={movie.posterUrl}
      thumbhash={movie.posterThumbhash}
      title={movie.title}
      detail={movie.year?.toString()}
    />
  );
  return (
    <li className="flex flex-col">
      {movie.libraryId === undefined ? (
        <Link to="/movies/$movieId" params={{ movieId }} className={posterLink}>
          {poster}
        </Link>
      ) : (
        <Link
          to="/libraries/$libraryId/movies/$movieId"
          params={{ libraryId: String(movie.libraryId), movieId }}
          className={posterLink}
        >
          {poster}
        </Link>
      )}
      {/* The button goes under the label: a poster's column is too narrow for both. */}
      <div className="mt-2 flex flex-col items-start gap-1">
        <FoundMovieState movie={movie} you={you} />
      </div>
    </li>
  );
}
