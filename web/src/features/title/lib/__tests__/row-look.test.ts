import { create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import { cardArt, rowLook } from "@/features/title/lib/row-look";
import type { RowEpisode } from "@/features/title/lib/show-row";
import { EpisodeSchema } from "@/gen/nama/v1/browse_pb";

const episode = (episodeNumber: number, overview?: string): RowEpisode => ({
  kind: "here",
  seasonNumber: 1,
  episodeNumber,
  episode: create(EpisodeSchema, { seasonNumber: 1, episodeNumber, overview }),
});

describe("a Show's row of Episodes", () => {
  const backdrop = { url: "/backdrop.jpg", thumbhash: new Uint8Array([1]) };

  test("is square throughout when any Episode has a synopsis", () => {
    expect(rowLook([episode(1, "A heist begins."), episode(2)], backdrop).tall).toBe(true);
  });

  test("is 4:3 throughout when no Episode has a synopsis", () => {
    expect(rowLook([episode(1), episode(2)], backdrop).tall).toBe(false);
  });

  test("shows an Episode's Still when it has one", () => {
    const still = { url: "/still.jpg", thumbhash: undefined };
    expect(cardArt(still, rowLook([], backdrop))).toStrictEqual(still);
  });

  test("shows the Show's backdrop for an Episode without a Still", () => {
    const none = { url: undefined, thumbhash: undefined };
    expect(cardArt(none, rowLook([], backdrop))).toStrictEqual(backdrop);
  });
});
