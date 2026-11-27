import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { TitleKind } from "@/gen/nama/v1/match_pb";
import { admin, fakeServer } from "@/test-server";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

test("needs a Match plays each Unmatched Item's Version", async () => {
  const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };
  const item = {
    itemId: 3n,
    versionId: 7n,
    kind: TitleKind.MOVIE,
    title: "Alien",
    year: 1979,
    path: "/srv/films/Alien (1979).mkv",
  };
  openSettings(
    "needs-a-match",
    fakeServer(
      { signedIn: admin, libraries: [films] },
      { MatchService: { listUnmatchedItems: () => ({ items: [item] }) } },
    ).transport,
  );

  const play = await screen.findByRole("link", { name: "Play Alien (1979)" });

  expect(play.getAttribute("href")).toBe("/libraries/1/unmatched/7/play");
});
