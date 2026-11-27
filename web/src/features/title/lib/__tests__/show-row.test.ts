import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import { heroTarget } from "@/features/title/lib/hero-target";
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
const twoSeasons = (inLibrary: number[]) => {
  const season = (seasonNumber: number) => {
    const episode = (episodeNumber: number) => ({
      episodeNumber,
      inLibrary: inLibrary.includes(seasonNumber),
    });
    return { seasonNumber, episodes: [episode(1), episode(2)] };
  };
  return found([season(0), season(1), season(2)]);
};
const places = (row: ReturnType<typeof showRow>) =>
  row.map(({ kind, seasonNumber, episodeNumber }) => `${kind} ${seasonNumber}x${episodeNumber}`);

describe("a Show's row", () => {
  test("holds the Library's Episodes and the missing ones by Season and Episode, Specials last", () => {
    const row = showRow([here(1, 1), here(1, 2)], twoSeasons([1]));
    expect(places(row)).toStrictEqual([
      "here 1x1",
      "here 1x2",
      "missing 2x1",
      "missing 2x2",
      "missing 0x1",
      "missing 0x2",
    ]);
  });

  test("holds an Episode another Library has as elsewhere, and only the Library's without a Show found", () => {
    const elsewhere = showRow([], twoSeasons([2]));
    expect(places(elsewhere)).toStrictEqual([
      "missing 1x1",
      "missing 1x2",
      "elsewhere 2x1",
      "elsewhere 2x2",
      "missing 0x1",
      "missing 0x2",
    ]);
    const alone = showRow([here(1, 1)]);
    expect(places(alone)).toStrictEqual(["here 1x1"]);
  });

  test("leads to Requesting Season 1 when nothing's on the Server", () => {
    const none = showRow([], twoSeasons([]));
    expect(heroTarget(none, undefined, false)).toStrictEqual({
      kind: "ask",
      season: 1,
      standing: { kind: "nobodyAsked", can: "open" },
    });
  });

  test("leads to Requesting the next Season once the one before is watched", () => {
    const row = showRow([here(1, 1, true), here(1, 2, true)], twoSeasons([1]));
    expect(heroTarget(row, undefined, false)).toMatchObject({ kind: "ask", season: 2 });
  });

  test("plays the next Episode on the Server, or the Episode Continue Watching has", () => {
    const next = here(2, 1);
    const row = showRow([here(1, 1, true), here(1, 2, true), next], twoSeasons([1]));
    expect(heroTarget(row, undefined, false)).toStrictEqual({ kind: "play", episode: next });
    const resumed = here(1, 2);
    expect(heroTarget(row, resumed, false)).toStrictEqual({ kind: "play", episode: resumed });
  });

  test("plays the first Episode once every one is watched", () => {
    const first = here(1, 1, true);
    const row = showRow([first, here(1, 2, true)]);
    expect(heroTarget(row, undefined, false)).toStrictEqual({ kind: "play", episode: first });
  });

  test("skips a Season you can't act on, and a Failed one unless you may Retry it", () => {
    const yours = { id: 7n, state: RequestState.PENDING, requesters: [{ userId: you }] };
    const failed = { id: 8n, state: RequestState.FAILED, requesters: [{ userId: 2n }] };
    const show = found([
      { seasonNumber: 1, request: yours, episodes: [{ episodeNumber: 1 }] },
      { seasonNumber: 2, request: failed, episodes: [{ episodeNumber: 1 }] },
    ]);
    const row = showRow([], show, you);
    expect(heroTarget(row, undefined, false)).toBeUndefined();
    expect(heroTarget(row, undefined, true)).toMatchObject({
      kind: "ask",
      season: 2,
      standing: { kind: "stalled", request: { id: 8n } },
    });
  });

  test("asks for only the Seasons you haven't watched any of", () => {
    const show = found([{ seasonNumber: 1 }, { seasonNumber: 2 }, { seasonNumber: 3 }]);
    const row = showRow([here(1, 1, true), here(2, 1)], show, you);
    expect(unwatchedToAsk(show, row, you)).toStrictEqual([2, 3]);
  });
});
