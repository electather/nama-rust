import { useId } from "react";

import { TrailerLink } from "@/features/title/components/trailer-link";
import type { Trailer } from "@/gen/nama/v1/request_pb";

/**
 * A title's Trailers, each a link out to where it is hosted. Nothing is embedded or previewed, so
 * the page tells the host nothing until a User opens one (ADR 0081).
 */
export function TitleTrailers({ trailers }: { trailers: Trailer[] }) {
  const heading = useId();
  if (trailers.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby={heading}>
      <h2 id={heading} className="mb-3.5 text-lg font-semibold">
        Trailers
      </h2>
      <ul className="flex flex-wrap gap-2">
        {trailers.map((trailer) => (
          <TrailerLink key={trailer.url} trailer={trailer} />
        ))}
      </ul>
    </section>
  );
}
