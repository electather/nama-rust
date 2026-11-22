import { useSearch } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import type { Section } from "@/features/settings/lib/sections";
import { builtInText } from "@/features/settings/lib/setting-search";

// About as long as it takes to see where the pane landed.
const MARKED_MS = 1500;

/**
 * Lands on the setting `?setting=` names in the `pane` of `section`, once it has loaded: scrolls its
 * row into view, focuses the row rather than its control, and marks it for a moment (`styles.css`).
 * A row shown only sometimes lands on the row that shows it; a key no row has leaves the pane at
 * its top.
 */
export function useFoundSetting(pane: RefObject<HTMLElement | null>, section: Section) {
  const setting = useSearch({ strict: false, select: (search) => search.setting });
  // shortcut: choosing the setting already in the address again changes nothing, so it isn't
  // marked again. Key this on a count of choices if people do.
  useEffect(() => {
    const row =
      pane.current !== null && setting !== undefined ? rowOf(pane.current, section, setting) : null;
    return row === null ? undefined : markFound(row);
  }, [pane, section, setting]);
}

/** The row of `setting` in `pane`, or while it's hidden, of the setting that shows it. */
function rowOf(pane: HTMLElement, section: Section, setting: string) {
  const find = (key: string) =>
    pane.querySelector<HTMLElement>(`[data-setting="${CSS.escape(key)}"]`);
  const shownBy = builtInText(section, setting)?.shownBy;
  return find(setting) ?? (shownBy === undefined ? null : find(shownBy));
}

/** Scrolls to `row`, focuses it and marks it, returning what unmarks it early. */
function markFound(row: HTMLElement) {
  row.tabIndex = -1;
  row.scrollIntoView({ block: "nearest" });
  row.focus({ preventScroll: true });
  row.dataset["found"] = "";
  const unmark = () => {
    delete row.dataset["found"];
  };
  const timer = setTimeout(unmark, MARKED_MS);
  return () => {
    clearTimeout(timer);
    unmark();
  };
}
