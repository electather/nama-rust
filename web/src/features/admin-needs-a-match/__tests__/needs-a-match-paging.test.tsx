import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { type ListUnmatchedItemsRequest, MatchService, TitleKind } from "@/gen/nama/v1/match_pb";
import { admin, fakeServer, openApp } from "@/test-app";

afterEach(cleanup);

const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, folders: ["/srv/films"] };

function movie(itemId: bigint, title: string) {
  return { itemId, path: `/srv/films/${title}.mkv`, kind: TitleKind.MOVIE, title };
}

/**
 * A fake Server with the Library `films` listing one Unmatched Movie per title in `titles`, a page
 * each, with each page's index as the token that lists it. Each `ListUnmatchedItems` it gets goes
 * in `asked`.
 */
function pagingServer(titles: string[], asked: ListUnmatchedItemsRequest[]) {
  return fakeServer({ signedIn: admin }, {}, {}, (router) => {
    router.service(LibraryService, { listLibraries: () => ({ libraries: [films] }) });
    router.service(MatchService, {
      listUnmatchedItems: (request) => {
        asked.push(request);
        const index = Number(request.page?.token ?? "");
        const title = titles[index] ?? "";
        const next = index + 1 < titles.length ? String(index + 1) : "";
        return { items: [movie(BigInt(index), title)], page: { nextToken: next } };
      },
    });
  });
}

test("needs a Match lists a Library's next page of Items with Show more", async () => {
  const asked: ListUnmatchedItemsRequest[] = [];
  openApp("/admin/needs-a-match", pagingServer(["Alien", "Heat"], asked));

  fireEvent.click(await screen.findByRole("button", { name: "Show more" }));

  await expect(screen.findByText("/srv/films/Heat.mkv")).resolves.toBeDefined();
  expect(screen.getByText("/srv/films/Alien.mkv")).toBeDefined();
  expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();
  expect(asked.map((request) => request.filter?.libraryId)).toStrictEqual([films.id, films.id]);
});
