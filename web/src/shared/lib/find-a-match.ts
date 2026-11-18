import { Code } from "@connectrpc/connect";

import type { SetMatchResponse, TitleKind } from "@/gen/nama/v1/match_pb";
import { formText, refusal } from "@/shared/lib/forms";
import { episodeName } from "@/shared/lib/title-facts";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

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
  return refusal(error, "set the Match", {
    [Code.Unavailable]: noAnswer,
    [Code.NotFound]: { form: "This file is gone from the Library." },
  }).form;
}
