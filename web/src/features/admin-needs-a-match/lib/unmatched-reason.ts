import { TitleKind, type UnmatchedItem, UnmatchedReason } from "@/gen/nama/v1/match_pb";
import { episodeName } from "@/shared/lib/title-facts";
import { titleWord } from "@/shared/lib/title-word";
import { withYear } from "@/shared/lib/unmatched-names";

/** What of an Unmatched Item says why it is, and what to name it. */
export type Reasoned = Pick<
  UnmatchedItem,
  "reason" | "kind" | "title" | "year" | "season" | "episode" | "tiedTitles"
>;

const numberWords = ["", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine"];

// Ten and over read better as digits than as words.
function count(n: number) {
  return numberWords[n] ?? String(n);
}

/** The Episode a Show lacks, as "Season 1, Episode 2", or "such Episode" if its name gave no numbers. */
function wanted({ season, episode }: Reasoned) {
  return season === undefined || episode === undefined
    ? "such Episode"
    : episodeName(season, episode, "full");
}

// Tables, so a Server that knows a reason this app doesn't falls back to the line for none.
type Says = Partial<Record<UnmatchedReason, (item: Reasoned) => string>>;

const captions: Says = {
  [UnmatchedReason.AMBIGUOUS]: ({ tiedTitles }) =>
    `${count(tiedTitles.length)} titles fit this name`,
  [UnmatchedReason.NO_TITLE_FITS]: () => "No title fits this name",
  [UnmatchedReason.UNKNOWN_ID_HINT]: () => "Its ID Hint names no title",
  [UnmatchedReason.NOT_IN_SHOW]: ({ season }) =>
    season === undefined
      ? "This Episode isn’t in its Show"
      : `This Episode isn’t in Season ${season}`,
  [UnmatchedReason.CLEARED_BY_HAND]: () => "Unmatched by hand",
  [UnmatchedReason.NO_SEARCHING_SOURCE]: () => "No title source can search yet",
};

/** Why `item` is on Needs a Match in a few words, for the line under its name in a list. */
export function reasonCaption(item: Reasoned) {
  return captions[item.reason]?.(item) ?? "Not Matched yet";
}

const sentences: Says = {
  [UnmatchedReason.AMBIGUOUS]: (item) => `${tied(item)} Pick the one this file is.`,
  [UnmatchedReason.NO_TITLE_FITS]: ({ kind }) =>
    `No ${titleWord(kind)} fits this file’s name. Search for it, or rename the file or add an ID Hint.`,
  [UnmatchedReason.UNKNOWN_ID_HINT]: ({ kind }) =>
    `This file’s ID Hint names no ${titleWord(kind)} the title source has. Search for it instead.`,
  [UnmatchedReason.NOT_IN_SHOW]: (item) => `Its Show has no ${wanted(item)}. Pick another Show.`,
  [UnmatchedReason.CLEARED_BY_HAND]: ({ kind }) =>
    `Its Match was cleared by hand. Pick its ${titleWord(kind)} to Match it again.`,
  [UnmatchedReason.NO_SEARCHING_SOURCE]: () =>
    "None of its Library’s title sources can search for titles. Once one can, the next scan tries again.",
};

/** Why `item` is on Needs a Match as a sentence that says what to do, for its own screen. */
export function reasonSentence(item: Reasoned) {
  return (
    sentences[item.reason]?.(item) ??
    `Matching hasn’t decided about this file yet. Pick its ${titleWord(item.kind)} to Match it by hand.`
  );
}

// "en-GB" for "A, B or C", without the serial comma, as the rest of the app writes lists.
const titleList = new Intl.ListFormat("en-GB", { type: "disjunction" });

// Titles named and dated alike are told by how many there are; others by their names.
function tied({ kind, tiedTitles }: Reasoned) {
  const [first] = tiedTitles;
  const alike = tiedTitles.every(
    ({ title, year }) => title === first?.title && year === first.year,
  );
  if (first === undefined || !alike) {
    const names = tiedTitles.map(({ title, year }) => withYear(title, year));
    return `More than one ${titleWord(kind)} fits: ${titleList.format(names)}.`;
  }
  const many = `${count(tiedTitles.length)} ${titleWord(kind)}s called ${first.title}`;
  const verb = kind === TitleKind.SHOW ? "first aired" : "came out";
  return first.year === undefined ? `${many} fit.` : `${many} ${verb} in ${first.year}.`;
}
