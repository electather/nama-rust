import { create, fromJson } from "@bufbuild/protobuf";
import { expect, test } from "vite-plus/test";

import { MovieHitSchema, RequestSchema, RequestState } from "@/gen/nama/v1/request_pb";
import { rowTile } from "@/shared/components/row-poster/row-tiles";

/** Arrival, missing from the Server, with `more` as given. */
const arrival = (more: { request?: { state: RequestState }; libraryId?: bigint } = {}) =>
  create(MovieHitSchema, { id: { namespace: "tmdb", value: "329865" }, title: "Arrival", ...more });

test.each([
  { case: "nobody asked", movie: arrival(), badge: "Not on this Server" },
  {
    case: "Pending",
    movie: arrival({ request: { state: RequestState.PENDING } }),
    badge: "Pending",
  },
  { case: "Failed", movie: arrival({ request: { state: RequestState.FAILED } }), badge: "Failed" },
  {
    case: "Declined",
    movie: arrival({ request: { state: RequestState.DECLINED } }),
    badge: "Not on this Server",
  },
  {
    case: "from a newer Server",
    movie: arrival({ request: fromJson(RequestSchema, { state: 99 }) }),
    badge: "Unknown",
  },
])("a missing Movie $case is badged $badge", ({ movie, badge }) => {
  expect(rowTile({ case: "missingMovie", value: movie }).badge?.label).toBe(badge);
});

test.each([
  {
    case: "Partially Available",
    movie: arrival({ request: { state: RequestState.PARTIALLY_AVAILABLE } }),
  },
  { case: "in a Library", movie: arrival({ libraryId: 1n }) },
])("a missing Movie $case has no badge", ({ movie }) => {
  expect(rowTile({ case: "missingMovie", value: movie }).badge).toBeUndefined();
});
