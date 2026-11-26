import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { type SearchResponseSchema, SearchService } from "@/gen/nama/v1/search_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

type Part = MessageInitShape<typeof SearchResponseSchema>;

const alien = {
  case: "movie",
  value: { libraryId: 1n, id: { namespace: "tmdb", value: "348" }, title: "Alien" },
} as const;

/** Finds Alien for "alien", and never answers anything else. */
const findsOnlyAlien: ServiceImpl<typeof SearchService>["search"] = ({ query }) =>
  new ReadableStream<Part>({
    start(controller) {
      if (query === "alien") {
        controller.enqueue({ part: { case: "hits", value: { hits: [{ title: alien }] } } });
        controller.enqueue({ part: { case: "done", value: {} } });
        controller.close();
      }
    },
  });

test("the titles found stay listed while the next words are searched", async () => {
  openApp(
    "/code",
    fakeServer({ signedIn: newUser }, {}, {}, (router) => {
      router.service(SearchService, { search: findsOnlyAlien });
    }),
  );
  await screen.findByRole("button", { name: "Search this Server" });
  fireEvent.keyDown(document.body, { key: "k", metaKey: true });
  const field = await screen.findByRole("combobox", { name: "Command menu" });
  fireEvent.change(field, { target: { value: "alien" } });
  await screen.findByRole("option", { name: /^Alien/u });

  fireEvent.change(field, { target: { value: "aliens" } });
  await new Promise((resolve) => {
    setTimeout(resolve, 400);
  });

  expect(screen.getAllByRole("option").map((row) => row.textContent)).toStrictEqual(["AlienOpen"]);
});
