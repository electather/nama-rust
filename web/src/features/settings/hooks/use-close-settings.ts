import { useNavigate } from "@tanstack/react-router";
import { useCallback } from "react";

/** Closes Settings, leaving the page under it. */
export function useCloseSettings() {
  const navigate = useNavigate();
  return useCallback(() => {
    // ponytail: closing replaces the entry Settings opened in, so the one it opened from is still
    // behind it and Back lands on the same page once. Go back instead once history says Settings
    // was opened from this page.
    void navigate({
      to: ".",
      search: (rest) => ({
        ...rest,
        settings: undefined,
        plugin: undefined,
        user: undefined,
        library: undefined,
        setting: undefined,
      }),
      replace: true,
    });
  }, [navigate]);
}
