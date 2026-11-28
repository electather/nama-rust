import type { ReactNode } from "react";

import type { TitleListing } from "@/features/library/lib/title-listing";

/** What a Library's grid lists, and what `change` shows it as when the User sorts or filters. */
export interface TitlesProps {
  libraryId: bigint;
  listing: TitleListing;
  change: (listing: TitleListing) => void;
}

/** The pages of a paged list so far, as a grid reads them, from `usePaged`. */
export interface Paged<Page> {
  data: { pages: Page[] };
  hasNextPage: boolean;
  /** Loads the page after the last; `cancelRefetch: false` asks once however often it's called. */
  fetchNextPage: (options?: { cancelRefetch?: boolean }) => Promise<unknown>;
  totalSize: number | undefined;
}

/** A Library's posters so far, as `usePaged` read them, with what they're called. */
export interface GridProps {
  titles: { one: string; many: string };
  paged: Paged<unknown>;
  posters: { key: string; poster: ReactNode }[];
  /** Turns the filters off, for a grid that none of the titles passed. */
  clear?: (() => void) | undefined;
}

/** The poster the quick-scroll rail asked the grid to scroll to, and what to call once it has. */
export interface Jump {
  target: number | undefined;
  reached: () => void;
}
