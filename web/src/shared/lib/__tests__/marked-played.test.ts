import { create } from "@bufbuild/protobuf";
import { describe, expect, test } from "vite-plus/test";

import { ListEpisodesResponseSchema } from "@/gen/nama/v1/browse_pb";
import { markEpisodes } from "@/shared/lib/marked-played";

describe("marking a Show's Episodes played", () => {
  const show = create(ListEpisodesResponseSchema, {
    episodes: [
      { id: { namespace: "tmdb", value: "1" }, seasonNumber: 1 },
      { id: { namespace: "tmdb", value: "2" }, seasonNumber: 2 },
    ],
  });
  const played = (data: typeof show) =>
    data.episodes.map(({ watchState }) => watchState?.played === true);

  test("a Season's mark marks only that Season's Episodes of a Show-wide list", () => {
    expect(played(markEpisodes(show, true, { seasonNumber: 2 }))).toStrictEqual([false, true]);
  });

  test("an Episode's mark marks only that Episode, and a Show's every one", () => {
    const one = { id: { namespace: "tmdb", value: "1" } };
    expect(played(markEpisodes(show, true, one))).toStrictEqual([true, false]);
    expect(played(markEpisodes(show, true))).toStrictEqual([true, true]);
  });
});
