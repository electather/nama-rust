import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const SHOW = "/libraries/2/shows/tmdb-70523";
const DURATION_S = 3000;
const titles = new Map([
  [3, "Past and Present"],
  [4, "Double Lives"],
  [5, "Truths"],
]);

/** A fake Server with Season 1 of Dark in Library 2, where Episode 4 plays after 3, if `hasNext`, and 5 after 4. */
function server(hasNext = true) {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getShow: () => ({ show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark" } }),
      listEpisodes: () => ({ episodes: [3, 4, 5].map((number) => episode(number)) }),
      getNextEpisode: ({ episodeNumber }) =>
        hasNext && episodeNumber < 5 ? { next: episode(episodeNumber + 1) } : {},
    });
    router.service(PlaybackService, {
      planPlayback: ({ versionId }) => ({
        plan: { method: PlaybackMethod.DIRECT_PLAY, url: `/media/versions/${versionId}` },
      }),
    });
    router.service(WatchStateService, { reportProgress: () => ({}) });
  });
}

function episode(episodeNumber: number) {
  return {
    id: { namespace: "tmdb", value: String(episodeNumber) },
    seasonNumber: 1,
    episodeNumber,
    title: titles.get(episodeNumber) ?? "",
    versions: [{ id: BigInt(episodeNumber), fileName: `Dark S01E0${episodeNumber}.mkv`, size: 1n }],
  };
}

/** Opens Episode 3's player with `search`, and finds its `<video>` once its length is known. */
async function playEpisode3(search = "", hasNext = true) {
  const router = openApp(`${SHOW}/seasons/1/episodes/3/play${search}`, server(hasNext));
  const video = await screen.findByLabelText<HTMLVideoElement>("Past and Present");
  Object.defineProperty(video, "duration", { value: DURATION_S });
  fireEvent(video, new Event("durationchange"));
  // The next Episode arrives beside the plan, so the countdown has it by its time.
  await waitFor(() => {
    expect(router.state.status).toBe("idle");
  });
  return { router, video };
}

function playUntilLeft(video: HTMLVideoElement, seconds: number) {
  video.currentTime = DURATION_S - seconds;
  fireEvent(video, new Event("timeupdate"));
}

function playToEnd(video: HTMLVideoElement) {
  playUntilLeft(video, 0);
  fireEvent(video, new Event("ended"));
}

const NEXT_EPISODE = { name: "Next Episode" };
const countdown = () => screen.queryByRole("complementary", NEXT_EPISODE);
const findCountdown = () => screen.findByRole("complementary", NEXT_EPISODE);

/** The path once any navigation `ended` started has finished. */
async function pathAfterNavigating(router: Awaited<ReturnType<typeof playEpisode3>>["router"]) {
  await act(async () => {});
  return router.state.location.pathname;
}

const EPISODE_4 = `${SHOW}/seasons/1/episodes/4/play`;

/** Where the app is once Episode 4's player opens. */
async function openedEpisode4(router: Awaited<ReturnType<typeof playEpisode3>>["router"]) {
  await screen.findByLabelText("Double Lives");
  return router.state.location;
}

test("the countdown shows the next Episode from 20 seconds before the end", async () => {
  const { video } = await playEpisode3();
  playUntilLeft(video, 21);
  expect(countdown()).toBeNull();

  playUntilLeft(video, 20);
  const shown = await findCountdown();
  expect(within(shown).getByText("Season 1, Episode 4")).toBeDefined();
  expect(within(shown).getByText("Double Lives")).toBeDefined();
  expect(within(shown).getByText("Starts in 0:20")).toBeDefined();
  expect(within(shown).getByRole("button", { name: "Play now" })).toBeDefined();
  expect(within(shown).getByRole("button", { name: "Cancel" })).toBeDefined();
});

test("Play now opens the next Episode", async () => {
  const { router, video } = await playEpisode3();
  playUntilLeft(video, 15);
  fireEvent.click(await screen.findByRole("button", { name: "Play now" }));

  expect((await openedEpisode4(router)).pathname).toBe(EPISODE_4);
});

test("the end of the Episode opens the next one", async () => {
  const { router, video } = await playEpisode3();
  playUntilLeft(video, 1);
  fireEvent(video, new Event("ended"));

  expect((await openedEpisode4(router)).pathname).toBe(EPISODE_4);
});

test("Cancel hides the countdown, and the Episode then just ends", async () => {
  const { router, video } = await playEpisode3();
  playUntilLeft(video, 10);
  fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
  expect(countdown()).toBeNull();

  playToEnd(video);
  expect(countdown()).toBeNull();
  expect(await pathAfterNavigating(router)).toBe(`${SHOW}/seasons/1/episodes/3/play`);
});

test("without a next Episode there's no countdown, and the end opens nothing", async () => {
  const { router, video } = await playEpisode3("", false);
  playUntilLeft(video, 5);
  expect(countdown()).toBeNull();

  fireEvent(video, new Event("ended"));
  expect(await pathAfterNavigating(router)).toBe(`${SHOW}/seasons/1/episodes/3/play`);
});

test("seeking back before the countdown hides it until it's reached again", async () => {
  const { video } = await playEpisode3();
  playUntilLeft(video, 20);
  expect(await findCountdown()).toBeDefined();

  playUntilLeft(video, 60);
  expect(countdown()).toBeNull();
  playUntilLeft(video, 19);
  expect(countdown()).not.toBeNull();
});

test("the countdown doesn't animate, so reduced motion has nothing to stop, and its seconds count", async () => {
  const { video } = await playEpisode3();
  playUntilLeft(video, 20);
  const shown = await findCountdown();
  expect(shown.querySelector('[class*="animate-"], [class*="duration-"]')).toBeNull();

  playUntilLeft(video, 12.4);
  expect(within(shown).getByText("Starts in 0:13")).toBeDefined();
});

test("after three Episodes autoplay started with no input, it pauses and asks", async () => {
  const { video } = await playEpisode3("?streak=3");
  await video.play();
  fireEvent(video, new Event("play"));
  playUntilLeft(video, 20);

  const question = await screen.findByRole("alertdialog", { name: "Are you still watching?" });
  expect(video.paused).toBe(true);
  expect(countdown()).toBeNull();

  fireEvent.click(within(question).getByRole("button", { name: "Keep watching" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(video.paused).toBe(false);
  expect(countdown()).not.toBeNull();
});

test("Back on the question opens the Show", async () => {
  const { router, video } = await playEpisode3("?streak=3");
  playUntilLeft(video, 20);
  const question = await screen.findByRole("alertdialog", { name: "Are you still watching?" });

  fireEvent.click(within(question).getByRole("link", { name: "Back" }));
  await waitFor(() => {
    expect(router.state.location.pathname).toBe(SHOW);
  });
});

test("autoplay with no input counts the Episodes in a row", async () => {
  const { router, video } = await playEpisode3("?streak=2");
  playToEnd(video);

  expect(await openedEpisode4(router)).toMatchObject({
    pathname: EPISODE_4,
    search: { streak: 3 },
  });
});

test.each([
  ["a pointer move", (video: HTMLElement) => fireEvent.pointerMove(video)],
  ["a pointer down", (video: HTMLElement) => fireEvent.pointerDown(video)],
  ["a key", () => fireEvent.keyDown(document, { key: "Shift" })],
])("%s during the Episode starts the count again", async (_input, giveInput) => {
  const { router, video } = await playEpisode3("?streak=2");
  giveInput(video);
  playToEnd(video);

  expect(await openedEpisode4(router)).toMatchObject({
    pathname: EPISODE_4,
    search: { streak: 1 },
  });
});

test("the next Episode counts its own input", async () => {
  const { router, video } = await playEpisode3("?streak=2");
  fireEvent.pointerMove(video);
  playToEnd(video);
  expect(await openedEpisode4(router)).toMatchObject({ search: { streak: 1 } });

  const episode4 = screen.getByLabelText<HTMLVideoElement>("Double Lives");
  fireEvent(episode4, new Event("ended"));
  await screen.findByLabelText("Truths");
  expect(router.state.location.search).toEqual({ streak: 2 });
});

test("input before a new Playback Plan still starts the count again", async () => {
  const { router, video } = await playEpisode3("?streak=2");
  fireEvent.pointerMove(video);
  fireEvent.error(video);
  await waitFor(() => {
    expect(screen.getByLabelText("Past and Present")).not.toBe(video);
  });

  const replanned = screen.getByLabelText<HTMLVideoElement>("Past and Present");
  Object.defineProperty(replanned, "duration", { value: DURATION_S });
  fireEvent(replanned, new Event("durationchange"));
  playToEnd(replanned);
  expect(await openedEpisode4(router)).toMatchObject({ search: { streak: 1 } });
});

test("nothing plays while the question waits for an answer", async () => {
  const { video } = await playEpisode3("?streak=3");
  playUntilLeft(video, 20);
  await screen.findByRole("alertdialog", { name: "Are you still watching?" });

  await video.play();
  fireEvent(video, new Event("play"));
  await waitFor(() => {
    expect(video.paused).toBe(true);
  });
  expect(screen.getByRole("alertdialog")).toBeDefined();
});

test("keys go to the question, not the player, so it can't be skipped", async () => {
  const { router, video } = await playEpisode3("?streak=3");
  playUntilLeft(video, 20);
  await screen.findByRole("alertdialog", { name: "Are you still watching?" });

  fireEvent.keyDown(document, { key: "ArrowRight" });
  expect(video.currentTime).toBe(DURATION_S - 20);
  fireEvent.keyDown(document, { key: " " });
  expect(video.paused).toBe(true);

  playToEnd(video);
  expect(await pathAfterNavigating(router)).toBe(`${SHOW}/seasons/1/episodes/3/play`);
  expect(screen.getByRole("alertdialog")).toBeDefined();
});

test("Esc on the question opens the Show", async () => {
  const { router, video } = await playEpisode3("?streak=3");
  playUntilLeft(video, 20);
  const question = await screen.findByRole("alertdialog", { name: "Are you still watching?" });

  fireEvent(question, new Event("cancel", { cancelable: true }));
  await waitFor(() => {
    expect(router.state.location.pathname).toBe(SHOW);
  });
});

test("Keep watching counts as input, so the count starts again", async () => {
  const { router, video } = await playEpisode3("?streak=3");
  playUntilLeft(video, 20);
  const question = await screen.findByRole("alertdialog", { name: "Are you still watching?" });
  fireEvent.click(within(question).getByRole("button", { name: "Keep watching" }));

  playToEnd(video);
  expect(await openedEpisode4(router)).toMatchObject({ search: { streak: 1 } });
});
