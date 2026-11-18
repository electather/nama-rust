import { useSyncExternalStore } from "react";

/** Whether something is full screen. */
export function useIsFullscreen() {
  return useSyncExternalStore(subscribeToFullscreen, () => document.fullscreenElement !== null);
}

function subscribeToFullscreen(onChange: () => void) {
  document.addEventListener("fullscreenchange", onChange);
  return () => {
    document.removeEventListener("fullscreenchange", onChange);
  };
}
