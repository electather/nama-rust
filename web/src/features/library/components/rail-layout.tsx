import type { ReactNode } from "react";

/**
 * A Library's grid with the column for its quick-scroll `rail` at its start. The column is there
 * in every sort and tab, empty when there's no rail, so the posters never move between them.
 */
export function RailLayout({ rail, children }: { rail?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-start gap-2.5">
      {/* Below the top bar, 72px (`h-18`), and 24px more. It hangs into the page's gutter. */}
      <div className="sticky top-24 -ms-3 w-8 flex-none sm:-ms-2.5 sm:w-10">{rail}</div>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}
