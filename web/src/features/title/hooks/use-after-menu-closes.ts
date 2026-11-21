import { createContext, useContext } from "react";

/**
 * Has a `MoreActions` menu do `action` once it has closed and handed focus back to its "…", such
 * as open a dialog, which then hands focus back there too rather than to the item that's gone.
 */
export const AfterMenuCloses = createContext<((action: () => void) => void) | undefined>(undefined);

/** Runs an action once the menu this is in has closed, or at once outside one. */
export function useAfterMenuCloses() {
  return (
    useContext(AfterMenuCloses) ??
    ((action: () => void) => {
      action();
    })
  );
}
