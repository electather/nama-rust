import { type ReactElement, createContext, use } from "react";

/**
 * The crumb every trail in Settings' header starts with, if any: on a phone, the way back from a
 * section's page to the list of sections. A context, so the trails sections build themselves, such
 * as Plugins over the Plugin open, start with it too.
 */
export const CrumbsRoot = createContext<ReactElement | undefined>(undefined);

/** The crumb trails start with here, if any. */
export function useCrumbsRoot(): ReactElement | undefined {
  return use(CrumbsRoot);
}
