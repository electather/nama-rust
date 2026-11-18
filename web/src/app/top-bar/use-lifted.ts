import { useSyncExternalStore } from "react";

function subscribe(onChange: () => void) {
  globalThis.addEventListener("scroll", onChange, { passive: true });
  return () => {
    globalThis.removeEventListener("scroll", onChange);
  };
}

/** Whether the page has scrolled past 8px, when the top bar lifts into its tray. */
export function useLifted() {
  return useSyncExternalStore(subscribe, () => globalThis.scrollY > 8);
}
