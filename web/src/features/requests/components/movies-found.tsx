import { Found } from "@/features/requests/components/found";
import { MovieCard } from "@/features/requests/components/movie-card";
import { useSearchMovies } from "@/features/requests/data/use-search-movies";
import { titleParam } from "@/shared/lib/title-param";

/** The Movies found for `title`, each with where it is on its way here. */
export function MoviesFound({ title, you }: { title: string; you: bigint | undefined }) {
  const search = useSearchMovies(title);
  return (
    <Found kind="Movies" title={title} search={search}>
      {search.data?.movies.map((movie) => (
        <MovieCard key={titleParam(movie.id)} movie={movie} you={you} />
      ))}
    </Found>
  );
}
