import { useId } from "react";

import { TrailerCard } from "@/features/title/components/trailer-card";
import type { Trailer } from "@/gen/nama/v1/request_pb";
import { shelfBleed } from "@/shared/lib/shelf-bleed";

/**
 * A title's Trailers, in one row of cards over the title's own `art` that scrolls sideways past the
 * page's column. Each plays in its host's player in a pop-up, loaded only once a User opens one, so
 * showing the page tells the host nothing (GLOSSARY.md, "Trailer").
 */
export function TitleTrailers({
  trailers,
  art,
}: {
  trailers: Trailer[];
  art: { url: string | undefined; thumbhash: Uint8Array | undefined };
}) {
  const heading = useId();
  if (trailers.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby={heading} className="flex flex-col gap-4">
      <h2 id={heading} className="text-lg font-semibold">
        Trailers
      </h2>
      {/* Overscroll stays on the row, so a swipe at its end doesn't go back a page. */}
      <ul
        className={`flex snap-x snap-mandatory scroll-px-shelf-edge scrollbar-none gap-5 overflow-x-auto overscroll-x-contain ${shelfBleed}`}
      >
        {trailers.map((trailer) => (
          <TrailerCard key={trailer.url} trailer={trailer} art={art} />
        ))}
      </ul>
    </section>
  );
}
