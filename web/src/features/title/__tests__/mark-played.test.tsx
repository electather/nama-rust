import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { layOutShelves } from "@/test-layout";
import { pickFromMenu } from "@/test-menu";

// A Show's row of Episodes is virtual, so it needs a layout to show its cards.
beforeEach(layOutShelves);
afterEach(cleanup);

const lies = {
  id: { namespace: "tmdb", value: "1388539" },
  seasonNumber: 1,
  episodeNumber: 2,
  title: "Lies",
  versions: [{ id: 1n }],
};

/** Dark's page, whose `SetPlayed` waits till `answer` is called. */
function openDark() {
  let answer: ((value: true) => void) | undefined;
  const asked = new Promise<true>((resolve) => {
    answer = resolve;
  });
  const transport = fakeServer(
    { signedIn: newUser, libraries: [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }] },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        listUnmatched: () => ({}),
        getShow: () => ({
          show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark" },
          seasons: [{ number: 1 }],
        }),
        listEpisodes: () => ({ episodes: [lies] }),
        listShows: () => ({ shows: [] }),
        listGenres: () => ({}),
      });
      router.service(WatchStateService, {
        setPlayed: async () => {
          await asked;
          return {};
        },
      });
    },
  );
  openApp("/libraries/2/shows/tmdb-70523", transport);
  return () => answer?.(true);
}

test("an Episode shows as watched as soon as it's marked, before the Server answers", async () => {
  const answer = openDark();

  await pickFromMenu("More for Lies", "Mark as watched");

  await waitFor(() => {
    expect(screen.getByText("Watched")).toBeDefined();
  });
  answer();
});
