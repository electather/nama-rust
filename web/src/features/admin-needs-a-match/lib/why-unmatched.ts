import { withYear } from "@/components/unmatched-item";
import { type UnmatchedItem, UnmatchedReason } from "@/gen/nama/v1/match_pb";

// "en-GB" for "A, B or C", without the serial comma, as the rest of the app writes lists.
const titleList = new Intl.ListFormat("en-GB", { type: "disjunction" });

const unmatchedBecause: Partial<Record<UnmatchedReason, string>> = {
  [UnmatchedReason.NO_TITLE_FITS]:
    "No title fits its name. Renaming the file or adding an ID Hint may help.",
  [UnmatchedReason.UNKNOWN_ID_HINT]: "Its ID Hint names no title.",
  [UnmatchedReason.NOT_IN_SHOW]:
    "Its Show has no such Episode. Matching it to another Show may help.",
  [UnmatchedReason.CLEARED_BY_HAND]: "Unmatched by hand.",
  [UnmatchedReason.NO_SEARCHING_SOURCE]:
    "None of its Library’s title sources can search for titles. Once one can, the next scan tries again.",
};

/** Why `item` is Unmatched, which says how to fix it, or "" if not known. */
export function whyUnmatched({ reason, tiedTitles }: UnmatchedItem) {
  if (reason !== UnmatchedReason.AMBIGUOUS) {
    return unmatchedBecause[reason] ?? "";
  }
  const tied = tiedTitles.map((title) => withYear(title.title, title.year));
  return `More than one title fits: ${titleList.format(tied)}.`;
}
