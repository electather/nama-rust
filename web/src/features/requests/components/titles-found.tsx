import { MoviesFound } from "@/features/requests/components/movies-found";
import { ShowsFound } from "@/features/requests/components/shows-found";
import { useTitlesFound } from "@/features/requests/data/use-titles-found";
import { type Searchable, searchedKind } from "@/features/requests/lib/searchable";
import { SearchNote } from "@/shared/components/search-note";

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
      <SearchNote note={search.note} className="mt-8 block text-sm empty:mt-0" />
    </>
  );
}
