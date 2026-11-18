import { Link } from "@tanstack/react-router";

import type { MovieInLibrary } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { posterLink } from "@/shared/lib/poster-classes";
import { yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A Movie in a grid of posters, with its year, opening it in its Library. */
export function MovieInLibraryItem({ movie }: { movie: MovieInLibrary }) {
  return (
    <li>
      <Link
        to="/libraries/$libraryId/movies/$movieId"
        params={{ libraryId: String(movie.libraryId), movieId: titleParam(movie.id) }}
        className={posterLink}
      >
        <Poster
          url={movie.posterUrl}
          thumbhash={movie.posterThumbhash}
          title={movie.title}
          detail={yearOf(movie.releaseDate)}
        />
      </Link>
    </li>
  );
}
