import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  PlaybackMethod,
  type PlaybackPlanSchema,
  PlaybackReasonKind,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";
import type { MessageInitShape } from "@bufbuild/protobuf";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

type Plan = MessageInitShape<typeof PlaybackPlanSchema>;
const tracks: Plan["tracks"] = [
  { codec: "h264", kind: { case: "video", value: { width: 1920, height: 1080 } } },
  { codec: "aac", language: "eng", kind: { case: "audio", value: { channels: 6 } } },
  { codec: "subrip", language: "fre", kind: { case: "subtitle", value: {} } },
];

/** A fake Server with The Matrix in Library 1, played by `plan`. */
const serverPlaying = (plan: Plan) => {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(PlaybackService, { planPlayback: () => ({ plan }) });
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 603n } },
          title: "The Matrix",
          versions: [{ id: 7n, fileName: "The Matrix (1999).mkv", size: 1n }],
        },
      }),
    });
  });
};

/** A Direct Play nothing stood in the way of. */
const server = serverPlaying({ method: PlaybackMethod.DIRECT_PLAY, url: "/planned/7", tracks });

/** As the Server words it, which the app's own wording replaces. */
const serverText = "The Server's own sentence";

/** A Remux that converts Dolby Digital Plus audio, with the Server busy besides. */
const remuxing = serverPlaying({
  method: PlaybackMethod.REMUX,
  url: "/media/plans/0/index.m3u8",
  tracks,
  reasons: [
    {
      kind: PlaybackReasonKind.DEVICE_CANT_DECODE_AUDIO,
      params: { codec: "eac3" },
      text: serverText,
      decided: true,
    },
    {
      kind: PlaybackReasonKind.SERVER_BUSY,
      params: { maxTranscodes: "2" },
      text: serverText,
    },
    {
      kind: PlaybackReasonKind.DEVICE_PLAYS_FEWER_CHANNELS,
      params: { channels: "2" },
      text: serverText,
      decided: true,
    },
    // How this app sees a kind added to the schema after it was built.
    { kind: PlaybackReasonKind.UNSPECIFIED, text: "Something new stood in the way", decided: true },
    // A reason missing the param its sentence needs.
    { kind: PlaybackReasonKind.TRANSCODES_ON_HARDWARE, text: "On some GPU" },
  ],
});

/** Opens the player for The Matrix on `fake` and its Playback info. */
async function openInfo(fake: ReturnType<typeof serverPlaying>) {
  openApp("/libraries/1/movies/tmdb-603/play", fake);
  fireEvent.click(await screen.findByRole("button", { name: "Playback info" }));
}

test("playback info says how the plan plays the Version and with which Tracks", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);

  const info = await screen.findByRole("button", { name: "Playback info" });
  expect(info.getAttribute("aria-expanded")).toBe("false");
  // A button, so Enter and Space open it from the keyboard as a click does.
  fireEvent.click(info);

  expect(info.getAttribute("aria-expanded")).toBe("true");
  const panel = within(screen.getByRole("region", { name: "Playback info" }));
  expect(panel.getByText("Direct Play")).toBeDefined();
  expect(panel.queryByRole("list")).toBeNull();
  expect(panel.queryByText("More")).toBeNull();
  expect(panel.getByText("H.264 · 1080p")).toBeDefined();
  expect(panel.getByText("AAC · 5.1 · English")).toBeDefined();
  expect(panel.getByText("SRT · French")).toBeDefined();
});

test("playback info leads with the reasons that decided the plan, in its own words, and folds the rest under More", async () => {
  await openInfo(remuxing);

  const region = screen.getByRole("region", { name: "Playback info" });
  const panel = within(region);
  expect(panel.getByText("Remux")).toBeDefined();
  const disclosure = region.querySelector("details");
  expect(disclosure?.open).toBe(false);
  fireEvent.click(panel.getByText("More"));
  expect(disclosure?.open).toBe(true);

  const lists = panel.getAllByRole("list").map((list) =>
    within(list)
      .getAllByRole("listitem")
      .map((item) => item.textContent),
  );
  expect(lists).toEqual([
    [
      "This Device can’t decode Dolby Digital Plus audio, so it can’t be Direct Played.",
      "Its audio is converted to Stereo, as this Device doesn’t play 5.1.",
      "Something new stood in the way",
    ],
    [
      "The Server is running as many Transcodes as it may at once (2), so it can’t Transcode this until one ends.",
      "On some GPU",
    ],
  ]);
  expect(panel.queryByText(serverText)).toBeNull();
});

test("Esc closes playback info and stays in the player, wherever focus is", async () => {
  await openInfo(server);

  // As after clicking the video, which takes no focus.
  fireEvent.keyDown(document.body, { key: "Escape" });

  expect(screen.queryByRole("region", { name: "Playback info" })).toBeNull();
  expect(screen.getByLabelText("The Matrix")).toBeDefined();
});

/** Every kind of Playback Reason the schema has, but the one for a kind this app doesn’t know. */
const kinds = Object.values(PlaybackReasonKind).filter(
  (kind) => typeof kind === "number" && kind !== PlaybackReasonKind.UNSPECIFIED,
);

test("playback info words every kind of reason the schema has, naming its params as the app does", async () => {
  const params = {
    codec: "hevc",
    container: "CONTAINER_MATROSKA",
    hdrFormat: "HDR_FORMAT_DOLBY_VISION",
    method: "PLAYBACK_METHOD_DIRECT_PLAY",
    maxTranscodes: "2",
    playbackLimit: "PLAYBACK_LIMIT_720P",
    widthPixels: "1280",
    heightPixels: "534",
    channels: "6",
    hardwareAcceleration: "HARDWARE_ACCELERATION_NVENC",
  };
  const reasons = kinds.map((kind) => ({ kind, params, text: serverText, decided: true }));
  await openInfo(
    serverPlaying({ method: PlaybackMethod.TRANSCODE, url: "/planned/7", tracks, reasons }),
  );

  const panel = within(screen.getByRole("region", { name: "Playback info" }));
  const worded = panel.getAllByRole("listitem").map((item) => item.textContent);
  expect(worded).toHaveLength(kinds.length);
  expect(worded).not.toContain(serverText);
  expect(worded).toEqual(
    expect.arrayContaining([
      "This Device can’t open Matroska files, so it can’t be Direct Played.",
      "This Device can’t decode HEVC video, so it can’t be Direct Played or Remuxed.",
      "This Device can’t show Dolby Vision, so it can’t be Direct Played or Remuxed.",
      "Direct Play failed on this Device.",
      "Its picture is larger than the 720p Playback Limit, so a Transcode scales it down to 1280×534.",
      "Its audio is converted to 5.1, the most a Remux or a Transcode makes.",
      "It Transcodes on the GPU with NVIDIA NVENC.",
    ]),
  );
});

test("a reason whose param names a value this app can't name shows the Server's words", async () => {
  const reasons = [
    {
      kind: PlaybackReasonKind.PLAYBACK_LIMIT_DROPPED,
      params: { playbackLimit: "PLAYBACK_LIMIT_8K" },
      text: serverText,
      decided: true,
    },
  ];
  await openInfo(
    serverPlaying({ method: PlaybackMethod.DIRECT_PLAY, url: "/planned/7", tracks, reasons }),
  );

  expect(screen.getByRole("listitem").textContent).toBe(serverText);
});

test("a plan that can't play lists the reasons that decided it, in this app's words where it has them", async () => {
  const reasons = [
    {
      kind: PlaybackReasonKind.DEVICE_CANT_DECODE_VIDEO,
      params: { codec: "hevc" },
      text: serverText,
      decided: true,
    },
    { kind: PlaybackReasonKind.UNSPECIFIED, text: "Something new stood in the way", decided: true },
    { kind: PlaybackReasonKind.HARDWARE_ACCELERATION_OFF, text: serverText },
  ];
  openApp("/libraries/1/movies/tmdb-603/play", serverPlaying({ tracks, reasons }));

  expect(await screen.findByText("This Version can’t play here")).toBeDefined();
  expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual([
    "This Device can’t decode HEVC video, so it can’t be Direct Played or Remuxed.",
    "Something new stood in the way",
  ]);
});
