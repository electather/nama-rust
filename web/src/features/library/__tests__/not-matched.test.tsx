import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, type ListUnmatchedResponseSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { TitleKind } from "@/gen/nama/v1/match_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];

/** A fake Server whose Library 1 holds no Movies and lists `unmatched` as its Unmatched Items. */
function libraryWith(unmatched: MessageInitShape<typeof ListUnmatchedResponseSchema>) {
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listMovies: () => ({}),
      listGenres: () => ({}),
      listUnmatched: () => unmatched,
    });
  });
}

test("a Library's Unmatched Items are folded under Not matched, by what their names say", async () => {
  openApp(
    "/libraries/1",
    libraryWith({
      items: [
        { versionId: 7n, kind: TitleKind.MOVIE, title: "Alien", year: 1979 },
        { versionId: 8n, kind: TitleKind.SHOW, title: "Severance", season: 1, episode: 2 },
      ],
      page: { totalSize: 2 },
    }),
  );

  const summary = await screen.findByText("Not matched");

  expect(summary.closest("details")?.open).toBe(false);
  expect(screen.getByText("2")).toBeDefined();
  expect(screen.getByText("Alien (1979)")).toBeDefined();
  expect(screen.getByText("Severance · Season 1, Episode 2")).toBeDefined();
});

test("a Library without Unmatched Items shows no Not matched", async () => {
  openApp("/libraries/1", libraryWith({}));

  await expect(screen.findByRole("heading", { name: "Films" })).resolves.toBeDefined();
  expect(screen.queryByText("Not matched")).toBeNull();
});
