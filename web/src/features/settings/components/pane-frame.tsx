import { type ReactNode, useId } from "react";

import { type Section, sectionTitles } from "@/features/settings/lib/sections";

/**
 * A pane of Settings: an `action` at its top end, if it has one, over its settings. The section's
 * name is in the list beside it, so here its title is only said.
 */
export function PaneFrame({
  section,
  action,
  children,
}: {
  section: Section;
  action?: ReactNode;
  children: ReactNode;
}) {
  const titleId = useId();
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-7.5 p-4 md:px-7.5 md:pt-5 md:pb-7.5"
    >
      <h3 id={titleId} className="sr-only">
        {sectionTitles[section]}
      </h3>
      <div className="flex justify-end empty:hidden">{action}</div>
      {/* Apart from the action, so a row that comes first has no hairline over it. */}
      <div className="flex flex-col gap-7.5">{children}</div>
    </section>
  );
}
