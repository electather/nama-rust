import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import { heroTarget, targetAsk } from "@/features/title/lib/hero-target";
import { unwatchedToAsk } from "@/features/title/lib/seasons-to-ask";
import { showRow } from "@/features/title/lib/show-row";
import { EpisodeSchema } from "@/gen/nama/v1/browse_pb";
import { type FoundSeasonSchema, FoundShowSchema, RequestState } from "@/gen/nama/v1/request_pb";

const you = 1n;
const here = (seasonNumber: number, episodeNumber: number, played = false) =>
  create(EpisodeSchema, {
    seasonNumber,
    episodeNumber,
    watchState: { played, positionMs: 0n },
  });
const found = (init: MessageInitShape<typeof FoundSeasonSchema>[]) =>
  create(FoundShowSchema, { title: "Severance", seasons: init });

// The page tests over the fake Server cover the row's order and the rest of its main button.
describe("a Show's row", () => {
  test("skips a Season you can't act on, and a Failed one unless you may Retry it", () => {
    const yours = { id: 7n, state: RequestState.PENDING, requesters: [{ userId: you }] };
    const failed = { id: 8n, state: RequestState.FAILED, requesters: [{ userId: 2n }] };
    const show = found([
      { seasonNumber: 1, request: yours, episodes: [{ episodeNumber: 1 }] },
      { seasonNumber: 2, request: failed, episodes: [{ episodeNumber: 1 }] },
    ]);
    const row = showRow([], show, you);
    expect(heroTarget(row, undefined, false)).toBeUndefined();
    const retried = targetAsk(heroTarget(row, undefined, true));
    expect(retried?.season).toBe(2);
    expect(retried?.standing.kind).toBe("stalled");
  });

  test("asks for only the Seasons you haven't watched any of", () => {
    const show = found([{ seasonNumber: 1 }, { seasonNumber: 2 }, { seasonNumber: 3 }]);
    const row = showRow([here(1, 1, true), here(2, 1)], show, you);
    expect(unwatchedToAsk(show, row, you)).toStrictEqual([2, 3]);
  });
});
