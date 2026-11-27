import { useSyncExternalStore } from "react";

import { phoneWidth } from "@/features/settings/lib/phone-width";

const wider = "(min-width: 48rem)";

function subscribe(changed: () => void) {
  const query = globalThis.matchMedia(wider);
  query.addEventListener("change", changed);
  return () => {
    query.removeEventListener("change", changed);
  };
}

/** Whether Settings is laid out for a phone now (`phoneWidth`), following the window's width. */
export function usePhoneWidth() {
  return useSyncExternalStore(subscribe, phoneWidth, () => false);
}
