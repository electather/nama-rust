import { Found } from "@/features/requests/components/found";
import type { MovieHit } from "@/gen/nama/v1/request_pb";
import { MovieCard } from "@/shared/components/title-cards/movie-card";
import { titleParam } from "@/shared/lib/title-param";

/** The Movies a `search` for `title` found, each with where it is on its way here. */
export function MoviesFound({
  title,
  search,
  you,
}: {
  title: string;
  search: { movies: MovieHit[]; searching: boolean; failed: boolean };
  you: bigint | undefined;
}) {
  return (
    <Found kind="Movies" title={title} search={search}>
      {search.movies.map((movie) => (
        <MovieCard key={titleParam(movie.id)} movie={movie} you={you} />
      ))}
    </Found>
  );
}
