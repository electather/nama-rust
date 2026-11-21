import type { MessageInitShape } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  PlaybackMethod,
  PlaybackService,
  type SegmentSchema,
  SegmentKind,
} from "@/gen/nama/v1/playback_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const SHOW = "/libraries/2/shows/tmdb-70523";
const DURATION_S = 3000;
const titles = new Map([
  [3, "Past and Present"],
  [4, "Double Lives"],
  [5, "Truths"],
]);

/**
 * A fake Server with Season 1 of Dark in Library 2, where Episode 4 plays after 3, if `hasNext`,
 * and 5 after 4, if `autoplay` is on. Each plan has `segments`.
 */
function server({
  hasNext = true,
  autoplay = true,
  segments = [],
}: {
  hasNext?: boolean;
  autoplay?: boolean;
  segments?: MessageInitShape<typeof SegmentSchema>[];
} = {}) {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getShow: () => ({ show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark" } }),
      listEpisodes: () => ({ episodes: [3, 4, 5].map((number) => episode(number)) }),
      getNextEpisode: ({ episodeNumber }) =>
        hasNext && episodeNumber < 5 ? { next: episode(episodeNumber + 1) } : {},
    });
    router.service(PlaybackService, {
      getAutoplay: () => ({ on: autoplay }),
      planPlayback: ({ versionId }) => ({
        plan: { method: PlaybackMethod.DIRECT_PLAY, url: `/media/versions/${versionId}`, segments },
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
async function playEpisode3(search = "", options: Parameters<typeof server>[0] = {}) {
  const router = openApp(`${SHOW}/seasons/1/episodes/3/play${search}`, server(options));
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
// oxlint-disable-next-line require-await -- it hands over the query's promise
const findCountdown = async () => screen.findByRole("complementary", NEXT_EPISODE);

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
  expect(within(shown).getByText("Next · S1, E4")).toBeDefined();
  expect(within(shown).getByText("Double Lives")).toBeDefined();
  expect(within(shown).getByText("Starts in 20s")).toBeDefined();
});

test("next Episode opens the next Episode without waiting", async () => {
  const { router, video } = await playEpisode3();
  playUntilLeft(video, 15);
  fireEvent.click(await screen.findByRole("button", { name: "Next Episode" }));

  const opened = await openedEpisode4(router);
  expect(opened.pathname).toBe(EPISODE_4);
});

test("the end of the Episode opens the next one", async () => {
  const { router, video } = await playEpisode3();
  playUntilLeft(video, 1);
  fireEvent(video, new Event("ended"));

  const opened = await openedEpisode4(router);
  expect(opened.pathname).toBe(EPISODE_4);
});

test("cancel hides the countdown, and the Episode then just ends", async () => {
  const { router, video } = await playEpisode3();
  playUntilLeft(video, 10);
  fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
  expect(countdown()).toBeNull();

  playToEnd(video);
  expect(countdown()).toBeNull();
  await expect(pathAfterNavigating(router)).resolves.toBe(`${SHOW}/seasons/1/episodes/3/play`);
});

test("without a next Episode there's no countdown, and the end opens nothing", async () => {
  const { router, video } = await playEpisode3("", { hasNext: false });
  playUntilLeft(video, 5);
  expect(countdown()).toBeNull();

  fireEvent(video, new Event("ended"));
  await expect(pathAfterNavigating(router)).resolves.toBe(`${SHOW}/seasons/1/episodes/3/play`);
});

test("with autoplay off there's no countdown or question, and the end opens nothing", async () => {
  const { router, video } = await playEpisode3("?streak=3", { autoplay: false });
  playUntilLeft(video, 20);
  expect(countdown()).toBeNull();
  expect(screen.queryByRole("alertdialog")).toBeNull();

  fireEvent(video, new Event("ended"));
  await expect(pathAfterNavigating(router)).resolves.toBe(`${SHOW}/seasons/1/episodes/3/play`);
});

test("seeking back before the countdown hides it until it's reached again", async () => {
  const { video } = await playEpisode3();
  playUntilLeft(video, 20);
  await expect(findCountdown()).resolves.toBeDefined();

  playUntilLeft(video, 60);
  expect(countdown()).toBeNull();
  playUntilLeft(video, 19);
  expect(countdown()).not.toBeNull();
});

test("the countdown counts the seconds left, rounded up", async () => {
  const { video } = await playEpisode3();
  playUntilLeft(video, 20);
  const shown = await findCountdown();

  playUntilLeft(video, 12.4);
  expect(within(shown).getByText("Starts in 13s")).toBeDefined();
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

test("back on the question opens the Show", async () => {
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

  await expect(openedEpisode4(router)).resolves.toMatchObject({
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

  await expect(openedEpisode4(router)).resolves.toMatchObject({
    pathname: EPISODE_4,
    search: { streak: 1 },
  });
});

test("the next Episode counts its own input", async () => {
  const { router, video } = await playEpisode3("?streak=2");
  fireEvent.pointerMove(video);
  playToEnd(video);
  await expect(openedEpisode4(router)).resolves.toMatchObject({ search: { streak: 1 } });

  const episode4 = screen.getByLabelText<HTMLVideoElement>("Double Lives");
  fireEvent(episode4, new Event("ended"));
  await screen.findByLabelText("Truths");
  expect({ ...router.state.location.search }).toStrictEqual({ streak: 2 });
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
  await expect(openedEpisode4(router)).resolves.toMatchObject({ search: { streak: 1 } });
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
  await expect(pathAfterNavigating(router)).resolves.toBe(`${SHOW}/seasons/1/episodes/3/play`);
  expect(screen.getByRole("alertdialog")).toBeDefined();
});

test("esc on the question opens the Show", async () => {
  const { router, video } = await playEpisode3("?streak=3");
  playUntilLeft(video, 20);
  const question = await screen.findByRole("alertdialog", { name: "Are you still watching?" });

  fireEvent(question, new Event("cancel", { cancelable: true }));
  await waitFor(() => {
    expect(router.state.location.pathname).toBe(SHOW);
  });
});

test("keep watching counts as input, so the count starts again", async () => {
  const { router, video } = await playEpisode3("?streak=3");
  playUntilLeft(video, 20);
  const question = await screen.findByRole("alertdialog", { name: "Are you still watching?" });
  fireEvent.click(within(question).getByRole("button", { name: "Keep watching" }));

  playToEnd(video);
  await expect(openedEpisode4(router)).resolves.toMatchObject({ search: { streak: 1 } });
});

/** Plays `video` to `seconds` in. */
function playTo(video: HTMLVideoElement, seconds: number) {
  video.currentTime = seconds;
  fireEvent(video, new Event("timeupdate"));
}

const CREDITS_S = 2800;
const withCredits = {
  segments: [{ kind: SegmentKind.CREDITS, startMs: BigInt(CREDITS_S * 1000) }],
};

test("with Credits the countdown starts with them, and offers to watch them", async () => {
  const { video } = await playEpisode3("", withCredits);
  playTo(video, CREDITS_S - 1);
  expect(countdown()).toBeNull();

  playTo(video, CREDITS_S);
  const shown = await findCountdown();
  expect(within(shown).getByText("Starts in 20s")).toBeDefined();
  expect(within(shown).getByRole("button", { name: "Watch credits" })).toBeDefined();
});

test("with Credits the next Episode opens when the countdown ends, before the video does", async () => {
  const { router, video } = await playEpisode3("", withCredits);
  playTo(video, CREDITS_S + 20);

  const opened = await openedEpisode4(router);
  expect(opened.pathname).toBe(EPISODE_4);
});

test("watch credits hides the countdown, and the Credits play on", async () => {
  const { router, video } = await playEpisode3("", withCredits);
  playTo(video, CREDITS_S + 5);
  fireEvent.click(await screen.findByRole("button", { name: "Watch credits" }));
  playTo(video, CREDITS_S + 25);

  expect(countdown()).toBeNull();
  await expect(pathAfterNavigating(router)).resolves.toBe(`${SHOW}/seasons/1/episodes/3/play`);
});

test("during an Intro or a Recap a button skips to where it ends, and none shows outside them", async () => {
  const { video } = await playEpisode3("", {
    segments: [
      { kind: SegmentKind.RECAP, startMs: 0n, endMs: 60_000n },
      { kind: SegmentKind.INTRO, startMs: 90_000n, endMs: 150_000n },
    ],
  });
  playTo(video, 10);
  fireEvent.click(await screen.findByRole("button", { name: "Skip recap" }));
  expect(video.currentTime).toBe(60);

  playTo(video, 70);
  expect(screen.queryByRole("button", { name: /^Skip (?:recap|intro)$/u })).toBeNull();

  playTo(video, 100);
  fireEvent.click(await screen.findByRole("button", { name: "Skip intro" }));
  expect(video.currentTime).toBe(150);
});
