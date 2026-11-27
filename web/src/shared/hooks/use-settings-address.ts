import { useSearch } from "@tanstack/react-router";

import { parseSettings } from "@/shared/lib/settings-address";

/** Where the address says Settings is: closed, or its page, detail, setting and picked Server. */
export function useSettingsAddress() {
  // Shared structurally, so a reader renders again only when the address changes.
  return useSearch({ strict: false, select: parseSettings, structuralSharing: true });
}
