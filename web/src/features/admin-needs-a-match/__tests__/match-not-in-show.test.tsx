import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { TitleKind, type UnmatchedItemSchema, UnmatchedReason } from "@/gen/nama/v1/match_pb";
import { admin, fakeServer } from "@/test-server";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

const shows = { id: 1n, name: "Shows", kind: LibraryKind.SHOWS, folders: ["/srv/shows"] };
const episode = {
  itemId: 7n,
  path: "/srv/shows/S10E01.mkv",
  kind: TitleKind.SHOW,
  title: "Bake Off",
};

test.each([
  ["alone", { unmatched: 1n }, ""],
  [
    "with how many other Episodes it Matched",
    { matched: 2n, unmatched: 1n },
    " 2 other Episodes Matched to it.",
  ],
])(
  "matching an Episode to a Show without it says so %s, and stays on its screen",
  async (_, answer, also) => {
    const listed: MessageInitShape<typeof UnmatchedItemSchema> = {
      ...episode,
      season: 10,
      episode: 1,
    };
    openSettings(
      "needs-a-match",
      fakeServer(
        { signedIn: admin, libraries: [shows] },
        {
          MatchService: {
            listUnmatchedItems: () => ({ items: [listed] }),
            searchTitles: () => ({
              candidates: [{ id: { namespace: "tmdb", value: "1" }, title: episode.title }],
            }),
            setMatch: () => {
              listed.reason = UnmatchedReason.NOT_IN_SHOW;
              return answer;
            },
          },
        },
      ).transport,
    );

    fireEvent.click(await screen.findByRole("link", { name: /^Bake Off/u }));
    await screen.findByRole("radio", { name: "Bake Off" });
    fireEvent.click(screen.getByRole("button", { name: "Match" }));

    const said = await screen.findByText(`That Show has no Season 10, Episode 1.${also}`);
    expect(screen.getAllByRole("status")).toContain(said);
    await expect(
      screen.findByText("Its Show has no Season 10, Episode 1. Pick another Show."),
    ).resolves.toBeDefined();
    expect(screen.getByRole("heading", { name: /^Bake Off/u, level: 3 })).toBeDefined();
  },
);
