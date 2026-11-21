import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RequestState, type ShowHitSchema } from "@/gen/nama/v1/request_pb";
import { TitleStatus } from "@/gen/nama/v1/search_pb";
import { fill, openApp } from "@/test-app";
import { searchServer } from "@/test-search";

afterEach(cleanup);

const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark", year: 2017 };

function showHit(value: MessageInitShape<typeof ShowHitSchema>, status: TitleStatus) {
  return { title: { case: "show" as const, value }, status };
}

/** Searches the Requests page for "Dark" and gives the Shows it found. */
async function searchDark() {
  await screen.findByLabelText("Find a Movie or Show");
  fill("Find a Movie or Show", "Dark");
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  return screen.findByRole("region", { name: "Shows for “Dark”" });
}

test("an Available Show opens its page in its Library", async () => {
  openApp(
    "/requests",
    searchServer([[showHit({ ...dark, libraryId: 2n }, TitleStatus.AVAILABLE)]]),
  );

  const shows = await searchDark();

  const link = await within(shows).findByRole("link", { name: /Dark/u });
  expect(link.getAttribute("href")).toBe("/libraries/2/shows/tmdb-70523");
});

test("a Requested Show shows its Season Request's pill and opens its request page", async () => {
  const request = { id: 3n, state: RequestState.PENDING };
  openApp("/requests", searchServer([[showHit({ ...dark, request }, TitleStatus.REQUESTED)]]));

  const shows = await searchDark();

  await expect(within(shows).findByText("Pending approval")).resolves.toBeDefined();
  const link = within(shows).getByRole("link", { name: /Dark/u });
  expect(link.getAttribute("href")).toBe("/shows/tmdb-70523");
});

test("a Show Requested only through its Followed Show says it's Requested", async () => {
  openApp("/requests", searchServer([[showHit(dark, TitleStatus.REQUESTED)]]));

  const shows = await searchDark();

  await expect(within(shows).findByText("Requested")).resolves.toBeDefined();
});
