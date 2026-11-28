import { create } from "@bufbuild/protobuf";
import {
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRouter,
} from "@tanstack/react-router";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { EpisodeCard } from "@/features/title/components/episode-card";
import { NotToldNotice } from "@/features/title/components/not-told-notice";
import { EpisodeSchema } from "@/gen/nama/v1/browse_pb";
import { ExternalIdSchema } from "@/gen/nama/v1/match_pb";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

// No ID, so the card holds no "…" and needs nothing but its link.
const lies = create(EpisodeSchema, { seasonNumber: 1, episodeNumber: 2, title: "Lies" });

function LiesCard() {
  return (
    <ul>
      <EpisodeCard
        show={{
          libraryId: 2n,
          showId: create(ExternalIdSchema, { namespace: "tmdb", value: "70 523" }),
        }}
        episode={lies}
        place={{ index: 0, of: 1 }}
        look={{ tall: false, backdrop: undefined }}
      />
    </ul>
  );
}

test("an Episode card off its Show's page plays the Show it's given", async () => {
  const router = createRouter({
    routeTree: createRootRoute({ component: LiesCard }),
    history: createMemoryHistory({ initialEntries: ["/libraries/9/shows/tmdb-1"] }),
  });
  render(<RouterProvider router={router} />, { wrapper: onServerAt() });

  const link = await screen.findByRole("link", { name: /^Play Season 1, Episode 2: Lies/u });
  expect(link.getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-70%2520523/seasons/1/episodes/2/play",
  );
});

test("the not-told notice names what a delete took and who may download it again", () => {
  render(<NotToldNotice deleted="Lies" notTold={["Sonarr"]} />);

  expect(
    screen.getByText("Sonarr couldn’t be told about the delete, so it may download Lies again."),
  ).toBeDefined();
});

test("the not-told notice shows nothing without a delete", () => {
  const { container } = render(<NotToldNotice deleted={undefined} notTold={undefined} />);

  expect(container.textContent).toBe("");
});
