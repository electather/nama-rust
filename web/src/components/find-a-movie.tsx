import { useQuery } from "@connectrpc/connect-query";
import { Link, useNavigate } from "@tanstack/react-router";

import { FormField } from "@/components/form-field";
import { Poster, PosterGrid, posterLink } from "@/components/poster";
import { FoundMovieState } from "@/components/request-state";
import { Button } from "@/components/ui/button";
import { type FoundMovie, RequestService } from "@/gen/nama/v1/request_pb";
import { formText, onSubmitted } from "@/lib/forms";

/** Searches the metadata source for a Movie to Request, through the Requests page's `?title=`. */
export function FindAMovie({ title }: { title: string | undefined }) {
  const navigate = useNavigate();
  return (
    <form
      // Keyed by the search, so going back to an earlier one shows what it searched for.
      key={title}
      className="mt-8 flex max-w-xl items-end gap-3"
      onSubmit={onSubmitted((typed) => {
        const wanted = formText(typed, "title").trim();
        void navigate({ to: "/requests", search: wanted === "" ? {} : { title: wanted } });
      })}
    >
      <div className="flex-1">
        <FormField
          label="Find a Movie"
          name="title"
          type="search"
          autoComplete="off"
          required={false}
          defaultValue={title}
        />
      </div>
      <Button type="submit">Search</Button>
    </form>
  );
}

/**
 * The Movies the metadata source found for `title`, each with where it is on its way here, as the
 * signed-in User `you` sees it.
 */
export function MoviesFound({ title, you }: { title: string; you: bigint | undefined }) {
  const search = useQuery(RequestService.method.searchMovies, { title });
  const heading = `Results for “${title}”`;
  return (
    <section aria-label={heading} className="mt-10">
      <h2 className="font-medium">{heading}</h2>
      {search.isPending && <p className="mt-8 text-muted-foreground">Searching…</p>}
      {search.isError && (
        <p role="alert" className="mt-8 text-muted-foreground">
          Couldn’t search for Movies. Try again in a moment.
        </p>
      )}
      {search.data && (
        <PosterGrid empty={<p className="mt-8 text-muted-foreground">No Movies found.</p>}>
          {search.data.movies.map((movie) => (
            <FoundCard key={String(movie.tmdbId)} movie={movie} you={you} />
          ))}
        </PosterGrid>
      )}
    </section>
  );
}

/** A Movie found: its poster opens its page, in its Library if one has it. */
function FoundCard({ movie, you }: { movie: FoundMovie; you: bigint | undefined }) {
  const movieId = `tmdb-${movie.tmdbId}`;
  const poster = (
    <Poster url={movie.posterUrl} title={movie.title} detail={movie.year?.toString()} />
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
