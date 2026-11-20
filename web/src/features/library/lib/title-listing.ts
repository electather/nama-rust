import { MovieSort, ShowSort, WatchedFilter } from "@/gen/nama/v1/browse_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";

/** What a Library's grid can sort by, as its URL names it, in the order the sort menu offers. */
export const sorts = { title: "Title", added: "Recently Added", released: "Release date" } as const;

/** The Watch States a Library's grid can keep, as its URL names them, in menu order. */
export const watchedStates = {
  "not-started": "Not started",
  started: "Started",
  played: "Watched",
} as const;

/** What a Library's grid is sorted by. */
export type Sort = keyof typeof sorts;
/** Which Watch State a Library's grid keeps titles in. */
export type Watched = keyof typeof watchedStates;

/** How a Library's grid is sorted and filtered, as its URL's search parameters hold it. */
export interface TitleListing {
  sort?: Sort | undefined;
  order?: "asc" | "desc" | undefined;
  genres?: string[] | undefined;
  from?: number | undefined;
  to?: number | undefined;
  watched?: Watched | undefined;
}

/** The listing `search` names, leaving out what it names wrongly, so a bad URL lists as usual. */
export function listingFromSearch(search: Record<string, unknown>): TitleListing {
  const { sort, order, genres, from, to, watched } = search;
  return {
    sort: keyIn(sorts, sort),
    order: keyIn(orders, order),
    genres: genresIn(genres),
    from: yearIn(from),
    to: yearIn(to),
    watched: keyIn(watchedStates, watched),
  };
}

const orders = { asc: "asc", desc: "desc" } as const;

function isKeyOf<T extends object>(record: T, value: unknown): value is keyof T & string {
  return typeof value === "string" && Object.hasOwn(record, value);
}

function keyIn<T extends object>(record: T, value: unknown) {
  return isKeyOf(record, value) ? value : undefined;
}

function genresIn(value: unknown) {
  const named = (Array.isArray(value) ? value : []).filter(
    (genre): genre is string => typeof genre === "string" && genre !== "",
  );
  return named.length === 0 ? undefined : named;
}

function yearIn(value: unknown) {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : undefined;
}

/** Whether `listing` keeps only some titles, rather than sorting them all. */
export function isFiltered({ genres, from, to, watched }: TitleListing) {
  return [genres, from, to, watched].some((value) => value !== undefined);
}

/** Which way `listing` runs: as its URL says, else up by title and newest first otherwise. */
export function isDescending({ sort = "title", order }: TitleListing) {
  return order === undefined ? sort !== "title" : order === "desc";
}

const movieSorts = {
  title: MovieSort.TITLE,
  added: MovieSort.ADDED,
  released: MovieSort.RELEASED,
};
const showSorts = { title: ShowSort.TITLE, added: ShowSort.ADDED, released: ShowSort.RELEASED };
const watchedFilters = {
  any: WatchedFilter.UNSPECIFIED,
  "not-started": WatchedFilter.NOT_STARTED,
  started: WatchedFilter.STARTED,
  played: WatchedFilter.PLAYED,
};

/**
 * The first page of `ListMovies` or `ListShows` for `listing`. 100 a page, so the 10 pages
 * `usePaged` holds are a 1,000-title Library.
 */
// ponytail: past 1,000 titles `usePaged` drops the first page as the 11th loads, so the grid
// jumps and its top can't be scrolled back to; keep the dropped rows' count, or page backwards,
// once Libraries grow past it.
export function firstPageOf(listing: TitleListing, kind: "movies" | "shows") {
  const { sort = "title", genres = [], from, to, watched = "any" } = listing;
  return {
    filter: {
      genres,
      minYear: from,
      maxYear: to,
      watched: watchedFilters[watched],
    },
    sort: { movies: movieSorts, shows: showSorts }[kind][sort],
    direction: isDescending(listing) ? SortDirection.DESCENDING : SortDirection.ASCENDING,
    page: { size: 100, token: "" },
  };
}
