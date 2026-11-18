import { Link } from "@tanstack/react-router";

import { Episodes } from "@/features/title/components/episodes";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * The Specials below a Show's only Season. Its heading links to their own page, which still offers
 * marking, deleting and the Poster of the Specials as a whole.
 */
export function Specials({ libraryId, showId }: { libraryId: bigint; showId: ExternalId }) {
  return (
    <section aria-labelledby="specials" className="mt-10">
      <h2 id="specials" className="font-medium">
        <Link
          from="/libraries/$libraryId/shows/$showId/"
          search={{ season: 0 }}
          className="rounded-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {seasonName(0)}
        </Link>
      </h2>
      <Episodes libraryId={libraryId} showId={showId} seasonNumber={0} />
    </section>
  );
}
