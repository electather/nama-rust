import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

beforeEach(() => {
  // Laid out, the panel's list shows 400px and each of its rows is 100px tall. Without a
  // ResizeObserver the list reads these; happy-dom's would measure 0px and show no rows.
  // oxlint-disable-next-line unicorn/no-useless-undefined -- `stubGlobal` takes the value to stub, here none
  vi.stubGlobal("ResizeObserver", undefined);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(100);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(400);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const SHOW = "/libraries/2/shows/tmdb-70523";
const titles = new Map([
  ["1/1", "Secrets"],
  ["1/2", "Lies"],
  ["1/3", "Past and Present"],
  ["2/1", "Beginnings and Endings"],
]);

function episode(seasonNumber: number, episodeNumber: number) {
  return {
    id: { namespace: "tmdb", value: `${seasonNumber}${episodeNumber}` },
    seasonNumber,
    episodeNumber,
    title: titles.get(`${seasonNumber}/${episodeNumber}`) ?? "",
    overview: `What happens in ${seasonNumber}/${episodeNumber}.`,
    runtimeMinutes: 52,
    // Episode 1 is played.
    watchState: { played: episodeNumber === 1 && seasonNumber === 1 },
    versions: [{ id: BigInt(seasonNumber * 10 + episodeNumber), fileName: "Dark.mkv", size: 1n }],
  };
}

/** A fake Server with two Seasons of Dark in Library 2, autoplay `on` or off. */
function server(on = true) {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getShow: () => ({
        show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark" },
        seasons: [{ number: 1 }, { number: 2 }],
      }),
      listEpisodes: ({ seasonNumber }) => ({
        episodes: (seasonNumber === 1 ? [1, 2, 3] : [1]).map((number) =>
          episode(seasonNumber, number),
        ),
      }),
      getNextEpisode: ({ episodeNumber }) =>
        episodeNumber < 3 ? { next: episode(1, episodeNumber + 1) } : { next: episode(2, 1) },
    });
    router.service(PlaybackService, {
      getAutoplay: () => ({ on }),
      planPlayback: ({ versionId }) => ({
        plan: { method: PlaybackMethod.DIRECT_PLAY, url: `/media/versions/${versionId}` },
      }),
    });
    router.service(WatchStateService, { reportProgress: () => ({}) });
  });
}

/** Opens Episode 2's player and its Episodes panel. */
async function openEpisodes(on = true) {
  const router = openApp(`${SHOW}/seasons/1/episodes/2/play`, server(on));
  fireEvent.click(await screen.findByRole("button", { name: "Episodes" }));
  const panel = await screen.findByRole("dialog", { name: "Episodes" });
  await within(panel).findByRole("link", { name: /Lies/u });
  return { router, panel };
}

test("the Episodes panel lists every Season's Episodes, marking the one playing and those played", async () => {
  const { panel } = await openEpisodes();

  expect(
    within(panel)
      .getAllByRole("heading")
      .map((heading) => heading.textContent),
  ).toStrictEqual(["Season 1", "Season 2"]);
  const rows = within(panel).getAllByRole("link");
  expect(rows.map((row) => row.textContent)).toStrictEqual([
    expect.stringContaining("Episode 1 · 52m · PlayedSecrets"),
    expect.stringContaining("Episode 2 · 52m · PlayingLies"),
    expect.stringContaining("Episode 3 · 52mPast and Present"),
    expect.stringContaining("Episode 1 · 52mBeginnings and Endings"),
  ]);
  expect(within(panel).getByRole("link", { current: "page" }).textContent).toContain("Lies");
  expect(within(panel).getByRole("tab", { name: "Season 1", selected: true })).toBeDefined();
});

test("a Season's tab scrolls to its start", async () => {
  const { panel } = await openEpisodes();
  // Laid out, the list has room to scroll any row to its top.
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(2000);
  const scrolled = vi.spyOn(HTMLElement.prototype, "scrollTo");

  fireEvent.click(within(panel).getByRole("tab", { name: "Season 2" }));

  expect(within(panel).getByRole("tab", { name: "Season 2", selected: true })).toBeDefined();
  // Season 2's heading is the list's fifth row, below four 100px rows and their 6px gaps, resting
  // under the list's 4px padding.
  expect(scrolled).toHaveBeenLastCalledWith({ top: 424, behavior: "smooth" });
  const row = within(panel).getByRole("link", { name: /Beginnings and Endings/u });
  expect(row.getAttribute("href")).toBe(`${SHOW}/seasons/2/episodes/1/play`);
});

test("picking an Episode plays it and closes the panel", async () => {
  const { router, panel } = await openEpisodes();

  fireEvent.click(within(panel).getByRole("link", { name: /Past and Present/u }));

  await screen.findByLabelText("Past and Present");
  expect(router.state.location.pathname).toBe(`${SHOW}/seasons/1/episodes/3/play`);
  expect(screen.queryByRole("dialog", { name: "Episodes" })).toBeNull();
});

test("next Episode plays the one after, previewing it, with autoplay off too", async () => {
  openApp(`${SHOW}/seasons/1/episodes/2/play`, server(false));

  const next = await screen.findByRole("link", { name: "Next Episode: Past and Present" });
  expect(next.getAttribute("href")).toBe(`${SHOW}/seasons/1/episodes/3/play`);
  expect(screen.getByRole("tooltip", { hidden: true }).textContent).toBe(
    "Episode 3 · 52mPast and Present",
  );
});
