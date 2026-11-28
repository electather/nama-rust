import type { TitleListing } from "@/features/library/lib/title-listing";

/** What a choice menu shows for no choice, which lists every title. */
export const ANY = "";

/** What a part of the bar changes: the listing, and what gets the listing changed. */
export interface BarProps {
  listing: TitleListing;
  change: (changed: TitleListing) => void;
}

/** What the Filter panel offers besides the bar's own: the Library's genres, and how many Unmatched Items it has. */
export interface FilterProps extends BarProps {
  genres: string[];
  /** How many Unmatched Items the Library has. */
  unmatched: number;
}
