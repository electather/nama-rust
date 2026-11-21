import { useState } from "react";

import { type FoundTitle, isFoundTitle } from "@/features/action-bar/lib/titles";

// Versioned, so a change to `FoundTitle` starts a new list instead of reading the old one wrong.
const storageKey = "nama:action-bar:recents:v1";
const kept = 4;

function read(): FoundTitle[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(storageKey) ?? "[]");
    return Array.isArray(stored)
      ? stored.filter((title) => isFoundTitle(title)).slice(0, kept)
      : [];
  } catch {
    // Storage that's blocked or holds something else means no Recent, not a broken action bar.
    return [];
  }
}

/** The titles last opened from the action bar on this Device, newest first, and how to add one. */
export function useRecents() {
  const [recents, setRecents] = useState(read);
  function remember(title: FoundTitle) {
    const next = [title, ...recents.filter(({ id }) => id !== title.id)].slice(0, kept);
    setRecents(next);
    try {
      // A thumbhash only stands in while its image loads, and its bytes don't survive JSON.
      const stored = JSON.stringify(next, (_, value: unknown) =>
        value instanceof Uint8Array ? undefined : value,
      );
      localStorage.setItem(storageKey, stored);
    } catch {
      // Blocked or full storage only costs Recent.
    }
  }
  return { recents, remember };
}
