import type { StillArt } from "@/features/action-bar/lib/titles";
import type { Request } from "@/gen/nama/v1/request_pb";

/** Where the action bar is: its root, a search of one kind of title, or the keyboard shortcuts. */
export type Frame = "root" | "movie" | "show" | "shortcuts";

/** How the action bar opens: where, and with what already typed. */
export interface Opening {
  frame: Frame;
  query: string;
}

/** The icons a row leads with, which `components/row-lead.tsx` draws. */
export type IconName = "movie" | "show" | "home" | "settings" | "keyboard" | "retry";

/** One thing the action bar lists, and what picking it does. */
export interface Row {
  /** Unique among the rows listed together, and without spaces: it ends a DOM id. */
  id: string;
  label: string;
  /** The line under the label, or `""` for none. */
  hint: string;
  icon?: IconName;
  /** A title's poster, or `null` for a title without one: its `icon` then fills the poster's box. */
  poster?: string | null;
  /** An Episode's Still, which it leads with in place of its `poster`. */
  still?: StillArt | undefined;
  /** What it is or is set to, in a pill beside the label. */
  badge?: string;
  /** A title's open Request, whose pill shows beside the label. */
  request?: Request;
  /** The keys that pick it from anywhere, beside the label. */
  keys?: readonly string[];
  /** The keys a keyboard shortcut's row is about, at its end. */
  chips?: readonly string[];
  /** What Enter does to it, in a word, shown while it's selected. */
  action?: string;
  /** Picks it. A keyboard shortcut's row only informs, so it has none. */
  run?: () => void;
}

/** Rows under one heading. */
export interface Group {
  heading: string;
  rows: Row[];
}

/** The `rows` a person typing `query` could mean. A blank `query` means every row. */
export function matching(query: string, rows: readonly Row[]) {
  const words = query.trim().toLowerCase();
  return rows.filter((row) => `${row.label} ${row.hint}`.toLowerCase().includes(words));
}

/** `rows` under `heading`, or no group at all when there are none. */
export function section(heading: string, rows: Row[]): Group[] {
  return rows.length === 0 ? [] : [{ heading, rows }];
}
