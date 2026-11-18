import type { TitleListing } from "@/features/library/lib/title-listing";

/** What a choice menu shows for no choice, which lists every title. */
export const ANY = "";

/** What a part of the bar changes: the listing, and what gets the listing changed. */
export interface BarProps {
  listing: TitleListing;
  change: (changed: TitleListing) => void;
}
