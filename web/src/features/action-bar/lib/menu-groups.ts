import type { FoundTitle } from "@/features/action-bar/lib/found-titles";
import {
  type Frame,
  type Group,
  type ListState,
  type Page,
  type Row,
  rowMatches,
} from "@/features/action-bar/lib/menu";
import { pageRow, searchModeRows, shortcutsRow, titleRow } from "@/features/action-bar/lib/rows";
import { shortcutGroups } from "@/features/action-bar/lib/shortcuts";

/** What the action bar lists: its frame, what's typed, and what it knows to list. */
export interface MenuState {
  frame: Frame;
  typed: string;
  pages: Page[];
  recents: FoundTitle[];
  /** What the Server found for the last search, which may lag what's typed. */
  found: FoundTitle[];
}

const kindsOf = {
  root: ["movie", "show", "episode"],
  movies: ["movie"],
  shows: ["show", "episode"],
  shortcuts: [],
} as const satisfies Record<Frame, FoundTitle["kind"][]>;

const resultHeadings = {
  movie: "Movie results",
  show: "Show results",
  episode: "Episode results",
} as const;

/** The groups the action bar lists, in order, leaving out any with no rows. */
export function menuGroups(state: MenuState): Group[] {
  const groups = state.frame === "shortcuts" ? shortcutsFound(state) : searchGroups(state);
  return groups.filter(({ rows }) => rows.length > 0);
}

function shortcutsFound({ pages, typed }: MenuState) {
  return shortcutGroups(pages).map((group) => ({
    heading: group.heading,
    rows: group.rows.filter((row) => rowMatches(row, typed)),
  }));
}

function searchGroups(state: MenuState): Group[] {
  const kinds: readonly FoundTitle["kind"][] = kindsOf[state.frame];
  const ofKind = (titles: FoundTitle[]) => titles.filter((title) => kinds.includes(title.kind));
  if (state.frame !== "root") {
    return state.typed === ""
      ? [{ heading: "Recent", rows: ofKind(state.recents).map((title) => titleRow(title)) }]
      : results(ofKind(state.found));
  }
  const typedRows = (rows: Row[]) => rows.filter((row) => rowMatches(row, state.typed));
  const jumpTo = { heading: "Jump to", rows: typedRows(state.pages.map((page) => pageRow(page))) };
  const actions = { heading: "Actions", rows: typedRows([shortcutsRow]) };
  if (state.typed === "") {
    const recent = { heading: "Recent", rows: state.recents.map((title) => titleRow(title)) };
    return [{ heading: "Search", rows: searchModeRows }, recent, jumpTo, actions];
  }
  return [jumpTo, ...results(state.found), actions];
}

function results(found: FoundTitle[]): Group[] {
  return (["movie", "show", "episode"] as const).map((kind) => ({
    heading: resultHeadings[kind],
    rows: found.filter((title) => title.kind === kind).map((title) => titleRow(title)),
  }));
}

/** Whether a search for what's typed is loading, while the last one's results stay up. */
export function isSearching({ frame, typed, searched }: ListState) {
  return frame !== "shortcuts" && typed !== "" && typed !== searched;
}

/** The index in the whole list of each group's first row. */
export function firstIndexes(groups: Group[]) {
  const firsts: number[] = [];
  let next = 0;
  for (const group of groups) {
    firsts.push(next);
    next += group.rows.length;
  }
  return firsts;
}
