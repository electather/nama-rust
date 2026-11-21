import { MoviesFound } from "@/features/requests/components/movies-found";
import { ShowsFound } from "@/features/requests/components/shows-found";
import { useTitlesFound } from "@/features/requests/data/use-titles-found";
import { type Searchable, searchedKind } from "@/features/requests/lib/searchable";

/**
 * The titles of the `kinds` a search for `title` found, on this Server and to Request, as the
 * signed-in User `you` sees them.
 */
export function TitlesFound({
  title,
  kinds,
  you,
}: {
  title: string;
  kinds: Searchable;
  you: bigint | undefined;
}) {
  const search = useTitlesFound(title, searchedKind(kinds));
  return (
    <>
      {kinds.movies && <MoviesFound title={title} search={search} you={you} />}
      {kinds.shows && <ShowsFound title={title} search={search} />}
      {/* Always there, so screen readers announce it as it fills. */}
      <output aria-live="polite" className="mt-8 block text-sm text-muted-foreground empty:mt-0">
        {search.titleSourceUnavailable
          ? "Titles to request couldn’t be searched, so only this Server’s are listed."
          : ""}
      </output>
    </>
  );
}
