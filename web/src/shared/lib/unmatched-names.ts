import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { episodeName } from "@/shared/lib/title-facts";

/**
 * What an Item's file name was parsed as, such as "Alien (1979)", "Severance · Season 1, Episode 2"
 * or "Severance · Specials, Episode 2".
 */
export function parsedAs(item: Pick<UnmatchedItem, "title" | "year" | "season" | "episode">) {
  const title = withYear(item.title, item.year);
  if (item.season === undefined || item.episode === undefined) {
    return title;
  }
  return `${title} · ${episodeName(item.season, item.episode, "full")}`;
}

/** `title`, followed by `year` in brackets if it's known, such as "Alien (1979)". */
export function withYear(title: string, year: number | undefined) {
  return year === undefined ? title : `${title} (${year})`;
}

/** What Matching an Item by hand needs of it: which Item, and what its file's name was parsed as. */
export type ItemToMatch = Pick<
  UnmatchedItem,
  "itemId" | "kind" | "title" | "year" | "season" | "episode"
>;
