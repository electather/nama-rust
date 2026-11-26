import type { Row } from "@/features/action-bar/lib/rows";
import type { FoundTitle } from "@/features/action-bar/lib/titles";

/** How many titles To request lists in the root list, as `search.proto` says every client does. */
export const toRequestListed = 6;

/**
 * `titles`: the title source's Movies and Shows taking turns, each kind in its own order (Movie 1,
 * Show 1, Movie 2, …), so one kind's many titles don't bury the other's best, then the Search
 * sources' in theirs.
 */
export function takingTurns(titles: readonly FoundTitle[]): FoundTitle[] {
  const fromTitleSource = titles.filter((title) => title.titleSource === true);
  const movies = fromTitleSource.filter((title) => title.kind === "movie");
  const shows = fromTitleSource.filter((title) => title.kind === "show");
  const turns = Array.from({ length: Math.max(movies.length, shows.length) }, (_, at) => [
    movies[at],
    shows[at],
  ]);
  return [
    ...turns.flat().filter((title) => title !== undefined),
    ...titles.filter((title) => title.titleSource !== true),
  ];
}

const kindsToRequest = ["movie", "show"] as const;
const plurals = { movie: "Movies", show: "Shows" } as const;

/** A row for each kind among `leftOut` that opens the search of that kind, which lists them all. */
export function moreToRequest(
  leftOut: readonly FoundTitle[],
  searchOf: (kind: (typeof kindsToRequest)[number]) => void,
): Row[] {
  return kindsToRequest
    .filter((kind) => leftOut.some((title) => title.kind === kind))
    .map((kind) => ({
      id: `more:${kind}`,
      label: `More ${plurals[kind]} to request`,
      hint: "",
      icon: kind,
      action: "Open",
      run: () => {
        searchOf(kind);
      },
    }));
}
