import { Found } from "@/features/requests/components/found";
import { useSearchMovies } from "@/features/requests/data/use-search-movies";
import { MovieCard } from "@/shared/components/title-cards/movie-card";
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
