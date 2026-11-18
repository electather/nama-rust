import type { ExternalId } from "@/gen/nama/v1/match_pb";

/** How a Movie's or Show's id is written in a URL, such as `tmdb-603`: the value is encoded. */
export function titleParam(id: ExternalId | undefined) {
  return `${id?.namespace}-${encodeURIComponent(id?.value ?? "")}`;
}
