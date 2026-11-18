import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  MatchService,
  TitleKind,
  type UnmatchedItemSchema,
  UnmatchedReason,
} from "@/gen/nama/v1/match_pb";
import { admin, fakeServer, openApp } from "@/test-app";

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
  "matching an Episode to a Show without it says so %s, and keeps it listed",
  async (_, answer, also) => {
    const listed: MessageInitShape<typeof UnmatchedItemSchema> = {
      ...episode,
      season: 10,
      episode: 1,
    };
    openApp(
      "/admin/needs-a-match",
      fakeServer({ signedIn: admin, libraries: [shows] }, {}, {}, (router) => {
        router.service(MatchService, {
          listUnmatchedItems: () => ({ items: [listed] }),
          searchTitles: () => ({
            candidates: [{ id: { namespace: "tmdb", value: "1" }, title: episode.title }],
          }),
          setMatch: () => {
            listed.reason = UnmatchedReason.NOT_IN_SHOW;
            return answer;
          },
        });
      }),
    );

    fireEvent.click(await screen.findByRole("button", { name: /^Find a Match for Bake Off/u }));
    fireEvent.click(await screen.findByRole("button", { name: "Match to Bake Off" }));

    const said = await screen.findByText(`That Show has no Season 10, Episode 1.${also}`);
    expect(screen.getAllByRole("status")).toContain(said);
    await expect(
      screen.findByText("Its Show has no such Episode. Matching it to another Show may help."),
    ).resolves.toBeDefined();
    expect(screen.getByText(episode.path)).toBeDefined();
  },
);
