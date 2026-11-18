import { MoviesFound } from "@/features/requests/components/movies-found";
import { ShowsFound } from "@/features/requests/components/shows-found";
import type { Searchable } from "@/features/requests/lib/searchable";

/** The titles of the `kinds` the title source found for `title`, as the signed-in User `you` sees them. */
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
