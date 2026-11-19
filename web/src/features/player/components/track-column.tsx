import { type ReactNode, useId } from "react";

/** One list of a panel, under its `heading`, which stays put as the list scrolls. */
export function TrackColumn({ heading, children }: { heading: string; children: ReactNode }) {
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className="min-h-0 scrollbar-quiet overflow-y-auto p-1">
      <h2 id={headingId} className="sticky -top-1 z-10 bg-popover px-2.5 pt-2 pb-2.5 font-semibold">
        {heading}
      </h2>
      {children}
    </section>
  );
}
