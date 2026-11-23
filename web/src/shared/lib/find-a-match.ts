import { Code } from "@connectrpc/connect";

import type { ExternalId, SetMatchResponse, TitleKind } from "@/gen/nama/v1/match_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, refusal } from "@/shared/lib/forms";
import { episodeName } from "@/shared/lib/title-facts";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/** A Matched Movie or Show whose every Item Find a Match Matches again, with what to search for first. */
export type TitleToMatch = Pick<ItemToMatch, "kind" | "title" | "year"> & {
  matchedTitle: { libraryId: bigint; id: ExternalId };
};

/**
 * What Find a Match Matches, the Library it is in, and what to do once Matched to `titleId`, such as
 * going to the title.
 */
export interface ToMatch {
  item: ItemToMatch | TitleToMatch;
  libraryId: bigint;
  onMatched?: ((titleId: ExternalId | undefined, matched: SetMatchResponse) => void) | undefined;
}

/** Whose Match Matching `toMatch` sets: its Item, and those named like it, or every Item of its title. */
export function matchTarget(toMatch: ItemToMatch | TitleToMatch) {
  return "matchedTitle" in toMatch
    ? { matchedTitle: toMatch.matchedTitle }
    : { itemId: toMatch.itemId };
}

/** What the title source is searched for, as the Find a Match form typed it. */
export function searchTyped(typed: FormData, kind: TitleKind) {
  const year = formText(typed, "year");
  return { kind, title: formText(typed, "title"), year: year === "" ? undefined : Number(year) };
}

/**
 * What Matching `item`, an Episode, to a Show without it did, or "" if the Show has it. The row
 * closes once its Item is Matched, so it shows this only while its own Episode is still Unmatched.
 */
export function notInShow(item: ItemToMatch, { matched, unmatched }: SetMatchResponse) {
  if (unmatched === 0n || item.season === undefined || item.episode === undefined) {
    return "";
  }
  return `That Show has no ${episodeName(item.season, item.episode, "full")}.${alsoMatched(matched)}`;
}

function alsoMatched(othersMatched: bigint) {
  if (othersMatched === 0n) {
    return "";
  }
  const others = othersMatched === 1n ? "1 other Episode" : `${othersMatched} other Episodes`;
  return ` ${others} Matched to it.`;
}

const noAnswer = { form: "The title source didn’t answer. Try again in a moment." };

/** Why searching the title source failed, in words. */
export function searchRefusal(error: unknown) {
  return refusal(error, "search", { [Code.Unavailable]: noAnswer }).form;
}

/** Why Matching failed, in words. */
export function setMatchRefusal(error: unknown) {
  return refusal(
    error,
    "set the Match",
    {
      [Code.Unavailable]: noAnswer,
      [Code.NotFound]: { form: "This file is gone from the Library." },
    },
    // Fix Match names a title, not a file.
    { [RefusalReason.NOTHING_MATCHED]: { form: "This title is gone from the Library." } },
  ).form;
}
