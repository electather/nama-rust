import type { ShowGroup } from "@/components/request-row";
import type { FollowedShow, Request } from "@/gen/nama/v1/request_pb";

/**
 * `requests` and `followedShows`, newest first, but with each Show's together where the Show's
 * newest one was: its Followed Shows, then its Season Requests in Season order.
 */
export function groupShows(requests: Request[], followedShows: FollowedShow[]) {
  const shows = new Map<string, ShowGroup>();
  const newestFirst = [...requests, ...followedShows];
  // Sorting is stable, so Requests opened in the same second keep the Server's order.
  newestFirst.sort((a, b) => Number(b.createdAt - a.createdAt));
  const grouped = newestFirst.flatMap((asked): (Request | FollowedShow | ShowGroup)[] => {
    const show = showOf(asked);
    if (show === undefined) {
      return [asked];
    }
    const known = shows.get(keyOf(show));
    const group = known ?? newGroup(show);
    addTo(group, asked);
    shows.set(keyOf(show), group);
    return known ? [] : [group];
  });
  for (const { seasons } of shows.values()) {
    seasons.sort((a, b) => seasonNumber(a) - seasonNumber(b));
  }
  return grouped;
}

export type Show = Pick<ShowGroup, "showId" | "showTitle">;

/** The Show a Followed Show or a Season Request is of; a Movie's Request has none. */
function showOf(asked: Request | FollowedShow): Show | undefined {
  if (asked.$typeName === "nama.v1.FollowedShow") {
    return asked;
  }
  const { movieOrSeason } = asked;
  return movieOrSeason.case === "season" ? movieOrSeason.value : undefined;
}

/** What tells `show` apart from other Shows. */
export function keyOf({ showId }: Show) {
  return `${showId?.namespace}-${showId?.value}`;
}

function newGroup({ showId, showTitle }: Show): ShowGroup {
  return { showId, showTitle, followed: [], seasons: [] };
}

function addTo(group: ShowGroup, asked: Request | FollowedShow) {
  if (asked.$typeName === "nama.v1.FollowedShow") {
    group.followed.push(asked);
  } else {
    group.seasons.push(asked);
  }
}

function seasonNumber(request: Request) {
  const { movieOrSeason } = request;
  return movieOrSeason.case === "season" ? movieOrSeason.value.seasonNumber : 0;
}
