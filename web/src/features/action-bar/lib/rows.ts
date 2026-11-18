import type { FoundTitle } from "@/features/action-bar/lib/found-titles";
import type { Page, Row } from "@/features/action-bar/lib/menu";
import { modKey } from "@/shared/lib/mod-key";

const badges = { movie: "Movie", show: "Show", episode: "Episode" } as const;

/** A found or recently opened title's row, opening its page. */
export function titleRow(title: FoundTitle): Row {
  return {
    key: title.key,
    label: title.title,
    hint: title.hint === "" ? undefined : title.hint,
    art: { icon: title.kind === "movie" ? "film" : "tv", posterUrl: title.posterUrl },
    badge: badges[title.kind],
    affordance: "Open",
    pick: { kind: "title", title },
  };
}

/** A page's row, jumping to it. */
export function pageRow(page: Page): Row {
  return {
    key: `page:${page.to}`,
    label: page.label,
    hint: page.hint,
    art: { icon: page.icon },
    affordance: "Go",
    pick: { kind: "page", page },
  };
}

/** The rows that narrow the search to one kind. */
export const searchModeRows: Row[] = [
  {
    key: "search:shows",
    label: "Search Shows",
    hint: "Find a Show or Episode in your Libraries",
    art: { icon: "tv" },
    affordance: "Open",
    pick: { kind: "frame", frame: "shows" },
  },
  {
    key: "search:movies",
    label: "Search Movies",
    hint: "Find a Movie in your Libraries",
    art: { icon: "film" },
    affordance: "Open",
    pick: { kind: "frame", frame: "movies" },
  },
];

/** The row that lists the keyboard shortcuts. */
export const shortcutsRow: Row = {
  key: "action:shortcuts",
  label: "Keyboard shortcuts",
  hint: "View every active hotkey",
  art: { icon: "keyboard" },
  hotkey: [modKey, "/"],
  affordance: "Run",
  pick: { kind: "frame", frame: "shortcuts" },
};
