import { useQuery } from "@connectrpc/connect-query";
import { Link, useNavigate } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { FormField } from "@/components/form-field";
import { Poster, PosterGrid, posterLink } from "@/components/poster";
import { FoundMovieState } from "@/components/found-movie-state";
import { Button } from "@/components/ui/button";
import { type FoundMovie, type FoundShow, RequestService } from "@/gen/nama/v1/request_pb";
import { formText, onSubmitted } from "@/lib/forms";

/** Which kinds of title the signed-in User may Request, and so search for. */
export interface Searchable {
  movies: boolean;
  shows: boolean;
}

/**
 * Searches the metadata source for the titles `kinds` names to Request, through the Requests
 * page's `?title=`.
 */
export function FindATitle({ title, kinds }: { title: string | undefined; kinds: Searchable }) {
  const navigate = useNavigate();
  const wanted = [kinds.movies && "Movie", kinds.shows && "Show"].filter(Boolean).join(" or ");
  return (
    <form
      // Keyed by the search, so going back to an earlier one shows what it searched for.
      key={title}
      className="mt-8 flex max-w-xl items-end gap-3"
      onSubmit={onSubmitted((typed) => {
        const typedTitle = formText(typed, "title").trim();
        void navigate({ to: "/requests", search: typedTitle === "" ? {} : { title: typedTitle } });
      })}
    >
      <div className="flex-1">
        <FormField
          label={`Find a ${wanted}`}
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

/** The titles of the `kinds` the metadata source found for `title`, as the signed-in User `you` sees them. */
export function TitlesFound({
  title,
  kinds,
  you,
}: {
  title: string;
  kinds: Searchable;
  you: bigint | undefined;
}) {
  return (
    <>
      {kinds.movies && <MoviesFound title={title} you={you} />}
      {kinds.shows && <ShowsFound title={title} />}
    </>
  );
}

/** The Movies found for `title`, each with where it is on its way here. */
function MoviesFound({ title, you }: { title: string; you: bigint | undefined }) {
  const search = useQuery(RequestService.method.searchMovies, { title });
  return (
    <Found kind="Movies" title={title} search={search}>
      {search.data?.movies.map((movie) => (
        <MovieCard key={String(movie.tmdbId)} movie={movie} you={you} />
      ))}
    </Found>
  );
}

/** The Shows found for `title`, each opening its page to pick Seasons. */
function ShowsFound({ title }: { title: string }) {
  const search = useQuery(RequestService.method.searchShows, { title });
  return (
    <Found kind="Shows" title={title} search={search}>
      {search.data?.shows.map((show) => (
        <ShowCard key={String(show.tmdbId)} show={show} />
      ))}
    </Found>
  );
}

/** The `kind` of titles a `search` for `title` found, once it's done: `children`, one card each. */
function Found({
  kind,
  title,
  search,
  children,
}: {
  kind: "Movies" | "Shows";
  title: string;
  search: { isPending: boolean; isError: boolean };
  children: ReactNode[] | undefined;
}) {
  const heading = `${kind} for “${title}”`;
  return (
    <section aria-label={heading} className="mt-10">
      <h2 className="font-medium">{heading}</h2>
      {search.isPending && <p className="mt-8 text-muted-foreground">Searching…</p>}
      {search.isError && (
        <p role="alert" className="mt-8 text-muted-foreground">
          Couldn’t search for {kind}. Try again in a moment.
        </p>
      )}
      {children && (
        <PosterGrid empty={<p className="mt-8 text-muted-foreground">No {kind} found.</p>}>
          {children}
        </PosterGrid>
      )}
    </section>
  );
}

function ShowCard({ show }: { show: FoundShow }) {
  return (
    <li>
      <Link to="/shows/$showId" params={{ showId: `tmdb-${show.tmdbId}` }} className={posterLink}>
        <Poster url={show.posterUrl} title={show.title} detail={show.year?.toString()} />
      </Link>
    </li>
  );
}

/** A Movie found: its poster opens its page, in its Library if one has it. */
function MovieCard({ movie, you }: { movie: FoundMovie; you: bigint | undefined }) {
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
