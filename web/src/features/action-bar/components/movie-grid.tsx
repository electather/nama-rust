import type { MovieInLibrary } from "@/gen/nama/v1/browse_pb";
import { PosterGrid } from "@/shared/components/poster/poster-grid";
import { MovieInLibraryItem } from "@/shared/components/titles-in-library/movie-in-library-item";
import { titleParam } from "@/shared/lib/title-param";

/** The Movies a search found, as posters. */
export function MovieGrid({ movies }: { movies: MovieInLibrary[] }) {
  return (
    <PosterGrid empty={null}>
      {movies.map((movie) => (
        <MovieInLibraryItem key={`${movie.libraryId}/${titleParam(movie.id)}`} movie={movie} />
      ))}
    </PosterGrid>
  );
}
