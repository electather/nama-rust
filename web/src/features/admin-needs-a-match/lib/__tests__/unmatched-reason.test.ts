import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import { reasonCaption, reasonSentence } from "@/features/admin-needs-a-match/lib/unmatched-reason";
import { TitleKind, UnmatchedItemSchema, UnmatchedReason } from "@/gen/nama/v1/match_pb";

/** An Unmatched Item: the Movie The Odyssey (2026) unless `init` says otherwise. */
function item(init: MessageInitShape<typeof UnmatchedItemSchema>) {
  return create(UnmatchedItemSchema, {
    kind: TitleKind.MOVIE,
    title: "The Odyssey",
    year: 2026,
    ...init,
  });
}

const tie = (year?: number) => ({ title: "The Odyssey", year });
const twoAlike = [tie(2026), tie(2026)];
const episode = {
  kind: TitleKind.SHOW,
  title: "Severance",
  year: undefined,
  season: 1,
  episode: 2,
};

describe("a reason in a few words", () => {
  test.each([
    [
      "two titles tie",
      { reason: UnmatchedReason.AMBIGUOUS, tiedTitles: twoAlike },
      "Two titles fit this name",
    ],
    [
      "twelve titles tie",
      {
        reason: UnmatchedReason.AMBIGUOUS,
        tiedTitles: Array.from({ length: 12 }, () => tie(2026)),
      },
      "12 titles fit this name",
    ],
    ["no title fits", { reason: UnmatchedReason.NO_TITLE_FITS }, "No title fits this name"],
    [
      "its ID Hint names no title",
      { reason: UnmatchedReason.UNKNOWN_ID_HINT },
      "Its ID Hint names no title",
    ],
    [
      "its Season isn't in the Show",
      { ...episode, reason: UnmatchedReason.NOT_IN_SHOW },
      "This Episode isn’t in Season 1",
    ],
    [
      "its Season is unknown",
      { kind: TitleKind.SHOW, reason: UnmatchedReason.NOT_IN_SHOW },
      "This Episode isn’t in its Show",
    ],
    ["it was cleared by hand", { reason: UnmatchedReason.CLEARED_BY_HAND }, "Unmatched by hand"],
    [
      "no source can search",
      { reason: UnmatchedReason.NO_SEARCHING_SOURCE },
      "No title source can search yet",
    ],
    ["Matching hasn't decided", {}, "Not Matched yet"],
  ])("%s", (_, init, caption) => {
    expect(reasonCaption(item(init))).toBe(caption);
  });
});

describe("a reason as a sentence", () => {
  test.each([
    [
      "alike Movies tie",
      { reason: UnmatchedReason.AMBIGUOUS, tiedTitles: twoAlike },
      "Two Movies called The Odyssey came out in 2026. Pick the one this file is.",
    ],
    [
      "alike Shows tie",
      {
        kind: TitleKind.SHOW,
        title: "Dark",
        reason: UnmatchedReason.AMBIGUOUS,
        tiedTitles: [
          { title: "Dark", year: 2017 },
          { title: "Dark", year: 2017 },
          { title: "Dark", year: 2017 },
        ],
      },
      "Three Shows called Dark first aired in 2017. Pick the one this file is.",
    ],
    [
      "alike Movies tie with no year",
      { reason: UnmatchedReason.AMBIGUOUS, tiedTitles: [tie(), tie()] },
      "Two Movies called The Odyssey fit. Pick the one this file is.",
    ],
    [
      "Movies tie that differ by year",
      { reason: UnmatchedReason.AMBIGUOUS, tiedTitles: [tie(2005), tie(2001)] },
      "More than one Movie fits: The Odyssey (2005) or The Odyssey (2001). Pick the one this file is.",
    ],
    [
      "no Movie fits",
      { reason: UnmatchedReason.NO_TITLE_FITS },
      "No Movie fits this file’s name. Search for it, or rename the file or add an ID Hint.",
    ],
    [
      "no Show fits",
      { ...episode, reason: UnmatchedReason.NO_TITLE_FITS },
      "No Show fits this file’s name. Search for it, or rename the file or add an ID Hint.",
    ],
    [
      "its ID Hint names no title",
      { reason: UnmatchedReason.UNKNOWN_ID_HINT },
      "This file’s ID Hint names no Movie the title source has. Search for it instead.",
    ],
    [
      "its Show lacks the Episode",
      { ...episode, reason: UnmatchedReason.NOT_IN_SHOW },
      "Its Show has no Season 1, Episode 2. Pick another Show.",
    ],
    [
      "its Show lacks an Episode of unknown number",
      { kind: TitleKind.SHOW, reason: UnmatchedReason.NOT_IN_SHOW },
      "Its Show has no such Episode. Pick another Show.",
    ],
    [
      "it was cleared by hand",
      { reason: UnmatchedReason.CLEARED_BY_HAND },
      "Its Match was cleared by hand. Pick its Movie to Match it again.",
    ],
    [
      "no source can search",
      { reason: UnmatchedReason.NO_SEARCHING_SOURCE },
      "None of its Library’s title sources can search for titles. Once one can, the next scan tries again.",
    ],
    [
      "Matching hasn't decided",
      {},
      "Matching hasn’t decided about this file yet. Pick its Movie to Match it by hand.",
    ],
  ])("%s", (_, init, sentence) => {
    expect(reasonSentence(item(init))).toBe(sentence);
  });
});
