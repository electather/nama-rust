import { TitleKind } from "@/gen/nama/v1/match_pb";
import type { FollowedShow, Request } from "@/gen/nama/v1/request_pb";

/** A Request's or a Followed Show's Quality, which approving it, or retrying a Request, can change. */
export interface QualityTarget {
  /** Whose Fulfiller's Qualities it can change to: a Movie's or a Show's. */
  kind: TitleKind;
  /** The id of the Quality it has now, if any. */
  id: bigint | undefined;
}

/**
 * What approving or retrying `request`, a Request or a Followed Show, can change its Quality to:
 * its Fulfiller's other Qualities. The Server refuses another for a Show once a Season of it was
 * handed off.
 */
export function qualityTargetOf(request: Request | FollowedShow): QualityTarget {
  const isMovie = request.$typeName === "nama.v1.Request" && request.movieOrSeason.case === "movie";
  return { kind: isMovie ? TitleKind.MOVIE : TitleKind.SHOW, id: request.quality?.id };
}
