import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** A fake Server with The Matrix in Library 1, Direct Played, with the Tracks the info panel lists. */
const server = (() => {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(PlaybackService, {
      planPlayback: () => ({
        plan: {
          method: PlaybackMethod.DIRECT_PLAY,
          url: "/planned/7",
          reasons: ["Its Tracks need no change"],
          tracks: [
            { codec: "h264", kind: { case: "video", value: { width: 1920, height: 1080 } } },
            { codec: "aac", language: "eng", kind: { case: "audio", value: { channels: 6 } } },
            { codec: "subrip", language: "fre", kind: { case: "subtitle", value: {} } },
          ] as const,
        },
      }),
    });
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
})();

test("playback info says how the plan plays the Version, why, and with which Tracks", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);

  const info = await screen.findByRole("button", { name: "Playback info" });
  expect(info.getAttribute("aria-expanded")).toBe("false");
  // A button, so Enter and Space open it from the keyboard as a click does.
  fireEvent.click(info);

  expect(info.getAttribute("aria-expanded")).toBe("true");
  const panel = within(screen.getByRole("region", { name: "Playback info" }));
  expect(panel.getByText("Direct Play")).toBeDefined();
  expect(panel.getByText("Its Tracks need no change")).toBeDefined();
  expect(panel.getByText("H.264 · 1080p")).toBeDefined();
  expect(panel.getByText("AAC · 5.1 · English")).toBeDefined();
  expect(panel.getByText("SRT · French")).toBeDefined();
});

test("Esc closes playback info and stays in the player, wherever focus is", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server);
  fireEvent.click(await screen.findByRole("button", { name: "Playback info" }));

  // As after clicking the video, which takes no focus.
  fireEvent.keyDown(document.body, { key: "Escape" });

  expect(screen.queryByRole("region", { name: "Playback info" })).toBeNull();
  expect(screen.getByLabelText("The Matrix")).toBeDefined();
});
