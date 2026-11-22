import { useNavigate } from "@tanstack/react-router";
import { useCallback } from "react";

import { sectionSearch } from "@/features/settings/lib/sections";

/** Closes Settings, leaving the page under it. */
export function useCloseSettings() {
  const navigate = useNavigate();
  return useCallback(() => {
    // ponytail: closing replaces the entry Settings opened in, so the one it opened from is still
    // behind it and Back lands on the same page once. Go back instead once history says Settings
    // was opened from this page.
    void navigate({
      to: ".",
      // sectionSearch lists every detail param a section opens, so closing clears them all.
      search: (rest) => ({ ...rest, ...sectionSearch("playback"), settings: undefined }),
      replace: true,
    });
  }, [navigate]);
}
