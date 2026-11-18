import type { FoundTitle } from "@/features/action-bar/lib/found-titles";

/** Which list the action bar shows: its root, a search of one kind, or the keyboard shortcuts. */
export type Frame = "root" | "movies" | "shows" | "shortcuts";

/** What the action bar opens on: a frame, and what's typed in it. */
export interface Opening {
  frame: Frame;
  query: string;
}

/** The icons a row can carry, drawn by `RowArt`. */
export type IconName =
  | "admin"
  | "film"
  | "home"
  | "keyboard"
  | "refresh"
  | "requests"
  | "settings"
  | "tv";

/** A page the action bar jumps to, by its row or by G then `letter`. */
export interface Page {
  label: string;
  hint: string;
  letter: string;
  icon: IconName;
  to: "/" | "/requests" | "/settings" | "/admin";
}

/** What picking a row does. */
export type Pick =
  | { kind: "frame"; frame: Frame }
  | { kind: "title"; title: FoundTitle }
  | { kind: "page"; page: Page }
  | { kind: "nothing" };

/** One row of the action bar's list. */
export interface Row {
  key: string;
  label: string;
  hint?: string | undefined;
  /** An icon, a poster (with the icon until there is one) or a blank. */
  art: { icon: IconName; posterUrl?: string | undefined } | undefined;
  badge?: string | undefined;
  /** The keys that run it from anywhere, beside its label. */
  hotkey?: string[];
  /** A shortcut the shortcuts list names, at the row's end. */
  chips?: string[];
  /** What Enter does, shown on the selected row. */
  affordance?: "Open" | "Go" | "Run";
  pick: Pick;
}

/** Rows under a heading. */
export interface Group {
  heading: string;
  rows: Row[];
}

/** Whether `row`'s label or hint holds what was `typed`, ignoring case. Nothing typed matches all. */
export function rowMatches(row: Row, typed: string) {
  return `${row.label} ${row.hint ?? ""}`.toLowerCase().includes(typed.toLowerCase());
}

/** What the list of the open action bar answers to the keys typed in its field. */
export interface ListHandle {
  count: number;
  pick: (index: number) => void;
}

/** What the list shows: as `MenuState`, but the search is run from `searched`. */
export interface ListState {
  frame: Frame;
  typed: string;
  /** What was typed when the search last ran, which lags `typed` while a search loads. */
  searched: string;
  pages: Page[];
  recents: FoundTitle[];
}
