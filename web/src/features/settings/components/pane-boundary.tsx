import type { ReactNode } from "react";

import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { type Section, loadNouns } from "@/features/settings/lib/sections";

/**
 * A pane's settings while they load, and in their place, if they don't, what couldn't load and a
 * way to try again. The rest of Settings stays as it is either way. A part that reads on its own
 * loads in a `SectionBoundary` of its own, so the pane shows without it.
 */
export function PaneBoundary({ section, children }: { section: Section; children: ReactNode }) {
  return (
    <SectionBoundary what={loadNouns[section]} rows={3} pane={section}>
      {children}
    </SectionBoundary>
  );
}
