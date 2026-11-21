import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  PlaybackMethod,
  type PlaybackPlanSchema,
  PlaybackService,
  SegmentKind,
} from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type Planned = Pick<
  MessageInitShape<typeof PlaybackPlanSchema>,
  "chapters" | "segments" | "previewFrames"
>;

const chapters = [
  { startMs: 0n, title: "" },
  { startMs: 3_000_000n, title: "The Construct" },
  { startMs: 6_000_000n, title: "Chapter 03" },
];
// The Credits start 3 s before the file's last Chapter, close enough to stand for it.
const credits = [{ kind: SegmentKind.CREDITS, startMs: 5_997_000n }];

/**
 * Opens The Matrix, Version 8, planned with `planned`, 7000 s long, and returns it and its Position
 * slider, put 400px wide, 100px from the left, since happy-dom lays nothing out.
 */
async function openPlanned(planned: Planned) {
  const plan = { method: PlaybackMethod.DIRECT_PLAY, url: "/planned/8", ...planned };
  const movie = {
    id: { namespace: "tmdb", value: "603" },
    title: "The Matrix",
    versions: [{ id: 8n, fileName: "The Matrix (1999).mkv", size: 1n }],
  };
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const server = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(PlaybackService, { planPlayback: () => ({ plan }) });
    router.service(BrowseService, { getMovie: () => ({ movie }) });
  });
  openApp("/libraries/1/movies/tmdb-603/play", server);
  const video = await screen.findByLabelText<HTMLVideoElement>("The Matrix");
  Object.defineProperty(video, "duration", { value: 7000 });
  fireEvent(video, new Event("durationchange"));
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 100, width: 400 }),
  );
  return { video, position: screen.getByLabelText("Position") };
}

/** The section the seek bar's hover card names beside `time`, empty for none. */
function cardNameBeside(time: string) {
  return screen.getByText(time).previousElementSibling?.textContent;
}

/** Which sheet the hover card's frame is copied from, and where in it, in pixels. */
function frameDrawn() {
  const canvas = document.querySelector("canvas");
  return [canvas?.dataset["sheet"], canvas?.dataset["at"]];
}

test("with Chapters, the hover card names the Chapter there, or the Segment that takes its place", async () => {
  const { position } = await openPlanned({ chapters, segments: credits });

  fireEvent.pointerMove(position, { clientX: 200 });
  expect(cardNameBeside("29:10")).toBe("Chapter 1");
  fireEvent.pointerMove(position, { clientX: 300 });
  expect(cardNameBeside("58:20")).toBe("The Construct");
  fireEvent.pointerMove(position, { clientX: 480 });
  expect(cardNameBeside("1:50:50")).toBe("Credits");
});

test("without Chapters, the Intro is its own section, and elsewhere the card says only the time", async () => {
  const { position } = await openPlanned({
    segments: [{ kind: SegmentKind.INTRO, startMs: 600_000n, endMs: 1_200_000n }],
  });

  fireEvent.pointerMove(position, { clientX: 150 });
  expect(cardNameBeside("14:35")).toBe("Intro");
  fireEvent.pointerMove(position, { clientX: 300 });
  expect(cardNameBeside("58:20")).toBe("");
});

test("the hover card shows the Preview frame there, cut from its sheet", async () => {
  const previewFrames = {
    intervalMs: 10_000,
    width: 320,
    height: 180,
    columns: 10,
    rows: 10,
    count: 700,
  };
  const { position } = await openPlanned({ previewFrames });

  // 58:20 is frame 350: sheet 3, the 51st on it, so column 1 of row 6 (counting from 1).
  fireEvent.pointerMove(position, { clientX: 300 });
  expect(frameDrawn()).toStrictEqual(["/media/versions/8/preview-frames/3.jpg", "0,900"]);
});

test("ctrl+Left and Right go to the previous and next section's start, even on the focused seek bar", async () => {
  const { video, position } = await openPlanned({ chapters, segments: credits });
  video.currentTime = 3500;

  fireEvent.keyDown(document, { key: "ArrowRight", ctrlKey: true });
  expect(video.currentTime).toBe(5997);
  fireEvent.keyDown(document, { key: "ArrowLeft", ctrlKey: true });
  expect(video.currentTime).toBe(3000);
  position.focus();
  fireEvent.keyDown(position, { key: "ArrowLeft", ctrlKey: true });
  expect(video.currentTime).toBe(0);
});

test("without sections, Ctrl+Left and Right do nothing", async () => {
  const { video } = await openPlanned({});
  video.currentTime = 3500;

  fireEvent.keyDown(document, { key: "ArrowRight", ctrlKey: true });
  fireEvent.keyDown(document, { key: "ArrowLeft", ctrlKey: true });
  expect(video.currentTime).toBe(3500);
});
