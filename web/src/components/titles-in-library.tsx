import { Link } from "@tanstack/react-router";

import { Poster, posterLink } from "@/components/poster";
import type { MovieInLibrary, ShowInLibrary } from "@/gen/nama/v1/browse_pb";
import { titleParam, yearOf } from "@/lib/browse";

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

/** A Show in a grid of posters, opening it in its Library. */
export function ShowInLibraryItem({ show }: { show: ShowInLibrary }) {
  return (
    <li>
      <Link
        to="/libraries/$libraryId/shows/$showId"
        params={{ libraryId: String(show.libraryId), showId: titleParam(show.id) }}
        className={posterLink}
      >
        <Poster url={show.posterUrl} thumbhash={show.posterThumbhash} title={show.title} />
      </Link>
    </li>
  );
}
