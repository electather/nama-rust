import type { RowEpisode } from "@/features/title/lib/show-row";
import type { FoundShow } from "@/gen/nama/v1/request_pb";
import { canAsk } from "@/shared/lib/can-ask";
import { isStarted } from "@/shared/lib/watch-progress";

/**
 * The numbered Seasons of `show` not in a Library that the signed-in User `you` can ask for, which
 * "Request All Seasons" asks for. Specials are left to their own cards.
 */
export function seasonsToAsk(show: FoundShow, you: bigint | undefined) {
  return show.seasons
    .filter(
      ({ seasonNumber, inLibrary, request }) =>
        seasonNumber > 0 && !inLibrary && canAsk(request, you),
    )
    .map(({ seasonNumber }) => seasonNumber);
}

/** Those of `seasonsToAsk` none of whose Episodes in `row` you've played or started. */
export function unwatchedToAsk(show: FoundShow, row: RowEpisode[], you: bigint | undefined) {
  const watched = new Set(
    row.flatMap((item) =>
      item.kind === "here" &&
      (item.episode.watchState?.played === true || isStarted(item.episode.watchState))
        ? [item.seasonNumber]
        : [],
    ),
  );
  return seasonsToAsk(show, you).filter((season) => !watched.has(season));
}
