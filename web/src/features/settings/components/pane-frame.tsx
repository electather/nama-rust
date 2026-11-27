import { type ReactNode, useId } from "react";

import { useOpenSection } from "@/features/settings/hooks/open-section";

/** A pane of Settings. The section's name is in the list beside it, so here its title is only said. */
export function PaneFrame({ children }: { children: ReactNode }) {
  const titleId = useId();
  const { title } = useOpenSection();
  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-4 p-4 md:px-5 md:pt-2 md:pb-6">
      <h3 id={titleId} className="sr-only">
        {title}
      </h3>
      <div className="flex flex-col gap-4">{children}</div>
    </section>
  );
}
