import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { type SearchResponseSchema, SearchService, TitleStatus } from "@/gen/nama/v1/search_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

/** A Server whose every search finds Alien and Sigourney Weaver. */
function server() {
  const hits = [
    {
      title: {
        case: "movie",
        value: { libraryId: 1n, id: { namespace: "tmdb", value: "348" }, title: "Alien" },
      },
      status: TitleStatus.AVAILABLE,
    },
    {
      title: {
        case: "person",
        value: { id: { namespace: "tmdb", value: "10205" }, name: "Sigourney Weaver" },
      },
      status: TitleStatus.AVAILABLE,
    },
  ] as const;
  return fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(SearchService, {
      search: () =>
        new ReadableStream<MessageInitShape<typeof SearchResponseSchema>>({
          start(controller) {
            controller.enqueue({ part: { case: "hits", value: { hits: [...hits] } } });
            controller.enqueue({ part: { case: "done", value: {} } });
            controller.close();
          },
        }),
    });
  });
}

test("a search lists the People it finds after the titles, and picking one opens their page", async () => {
  const router = openApp("/code", server());
  await screen.findByRole("button", { name: "Search this Server" });
  fireEvent.keyDown(document.body, { key: "k", metaKey: true });
  const field = await screen.findByRole("combobox", { name: "Command menu" });

  fireEvent.change(field, { target: { value: "sigourney" } });
  const weaver = await screen.findByRole("option", { name: /^Sigourney Weaver/u });

  expect(screen.getAllByRole("option").map((row) => row.textContent)).toStrictEqual([
    "AlienOpen",
    "Sigourney Weaver",
  ]);
  fireEvent.click(weaver);
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/people/tmdb-10205");
  });
});
