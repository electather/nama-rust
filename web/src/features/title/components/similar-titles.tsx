import { useId } from "react";

import { SimilarPoster } from "@/features/title/components/similar-poster";
import type { SimilarTitle } from "@/features/title/lib/similar-titles";

/**
 * A title's Similar titles that are in a Library, under `heading`, in one row of posters that
 * scrolls sideways past the page's column to the screen's edges, snapping to each. Nothing when
 * none is.
 */
export function SimilarTitles({ heading, titles }: { heading: string; titles: SimilarTitle[] }) {
  const headingId = useId();
  if (titles.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <h2 id={headingId} className="text-lg font-semibold">
        {heading}
      </h2>
      {/* Overscroll stays on the row, so a swipe at its end doesn't go back a page. */}
      <ul className="mx-bleed flex snap-x snap-mandatory scroll-px-shelf-edge scrollbar-none gap-5 overflow-x-auto overscroll-x-contain px-shelf-edge pt-1 pb-2">
        {titles.map((title) => (
          <SimilarPoster key={title.key} title={title} />
        ))}
      </ul>
    </section>
  );
}
