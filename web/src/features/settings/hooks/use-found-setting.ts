import { useSearch } from "@tanstack/react-router";
import { type RefObject, useEffect } from "react";

import type { Section } from "@/features/settings/lib/sections";
import { builtInText } from "@/features/settings/lib/setting-search";

// About as long as it takes to see where the pane landed.
const MARKED_MS = 1500;

/**
 * Lands on the setting `?setting=` names in the `pane` of `section`, once its row has loaded: scrolls its
 * row into view, focuses the row rather than its control, and marks it for a moment (`styles.css`).
 * A row shown only sometimes lands on the row that shows it; a key no row has leaves the pane at
 * its top.
 */
export function useFoundSetting(pane: RefObject<HTMLElement | null>, section: Section) {
  const setting = useSearch({ strict: false, select: (search) => search.setting });
  useEffect(() => {
    const root = pane.current;
    let unmark: (() => void) | undefined;
    const land = () => {
      const row = root === null || setting === undefined ? null : rowOf(root, section, setting);
      unmark = row === null ? undefined : markFound(row);
      return unmark !== undefined;
    };
    const observer = new MutationObserver(() => {
      if (land()) {
        observer.disconnect();
      }
    });
    // A part of the pane that loads on its own may bring the row in later.
    if (root !== null && setting !== undefined && !land()) {
      observer.observe(root, { childList: true, subtree: true });
    }
    return () => {
      observer.disconnect();
      unmark?.();
    };
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
