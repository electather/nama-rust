import { createContext, use } from "react";

import type { Section } from "@/shared/lib/settings-address";

/** The Section a pane, crumb or action belongs to, and its title. */
export interface OpenedSection {
  section: Section;
  title: string;
}

/**
 * The Section open, which Settings hands its pane, crumbs and action. Why it's a context is in
 * `web/src/AGENTS.md` ("Only Settings' modal tree imports the Section definitions").
 */
export const OpenSection = createContext<OpenedSection | undefined>(undefined);

/** The Section whose part calls this. Only a Section's parts may. */
export function useOpenSection(): OpenedSection {
  const open = use(OpenSection);
  if (open === undefined) {
    throw new Error("Only a Section's pane, crumbs and action know the Section open.");
  }
  return open;
}
