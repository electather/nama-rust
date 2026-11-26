import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import { heroTarget } from "@/features/title/lib/hero-target";
import { seasonsToAsk, unwatchedToAsk } from "@/features/title/lib/seasons-to-ask";
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
    expect(heroTarget(none)).toStrictEqual({
      kind: "ask",
      season: 1,
      request: undefined,
    });
  });

  test("leads to Requesting the next Season once the one before is watched", () => {
    const row = showRow([here(1, 1, true), here(1, 2, true)], twoSeasons([1]));
    expect(heroTarget(row)).toMatchObject({ kind: "ask", season: 2 });
  });

  test("plays the next Episode on the Server, or the Episode Continue Watching has", () => {
    const next = here(2, 1);
    const row = showRow([here(1, 1, true), here(1, 2, true), next], twoSeasons([1]));
    expect(heroTarget(row)).toStrictEqual({ kind: "play", episode: next });
    const resumed = here(1, 2);
    expect(heroTarget(row, resumed)).toStrictEqual({ kind: "play", episode: resumed });
  });

  test("plays the first Episode once every one is watched", () => {
    const first = here(1, 1, true);
    const row = showRow([first, here(1, 2, true)]);
    expect(heroTarget(row)).toStrictEqual({ kind: "play", episode: first });
  });

  test("leads to a Season's Request when it has one", () => {
    const request = { id: 7n, state: RequestState.PENDING, requesters: [{ userId: you }] };
    const show = found([{ seasonNumber: 1, request, episodes: [{ episodeNumber: 1 }] }]);
    expect(heroTarget(showRow([], show))).toMatchObject({
      kind: "ask",
      season: 1,
      request: { id: 7n },
    });
  });

  test("asks for every numbered Season not in a Library that you may ask for, or only those you haven't watched", () => {
    const show = found([
      { seasonNumber: 0 },
      { seasonNumber: 1, episodes: [{ episodeNumber: 1, inLibrary: true }, { episodeNumber: 2 }] },
      { seasonNumber: 2, request: { state: RequestState.PENDING, requesters: [{ userId: you }] } },
      { seasonNumber: 3, request: { state: RequestState.PENDING, requesters: [{ userId: 2n }] } },
      { seasonNumber: 4, inLibrary: true },
      { seasonNumber: 5, request: { state: RequestState.DECLINED, requesters: [{ userId: you }] } },
    ]);
    expect(seasonsToAsk(show, you)).toStrictEqual([1, 3, 5]);
    const row = showRow([here(1, 1, true)], show);
    expect(unwatchedToAsk(show, row, you)).toStrictEqual([3, 5]);
  });
});
