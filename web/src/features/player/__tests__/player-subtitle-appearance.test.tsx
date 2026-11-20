import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  PlaybackMethod,
  PlaybackService,
  SubtitleBackground,
  SubtitleSize,
} from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

test("the video sits in the User's Subtitle Appearance, which styles its cues", async () => {
  const transport = fakeServer(
    { signedIn: newUser, libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }] },
    {},
    {},
    (router) => {
      router.service(PlaybackService, {
        planPlayback: () => ({
          plan: { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7" },
        }),
        getSubtitleAppearance: () => ({
          size: SubtitleSize.LARGEST,
          background: SubtitleBackground.BOX,
        }),
      });
      router.service(BrowseService, {
        getMovie: () => ({
          movie: {
            id: { namespace: "tmdb", value: "603" },
            title: "The Matrix",
            versions: [{ id: 7n, fileName: "The Matrix (1999).mkv", size: 1n }],
          },
        }),
      });
    },
  );
  openApp("/libraries/1/movies/tmdb-603/play", transport);

  const video = await screen.findByLabelText("The Matrix");
  const appearance = () => {
    const styled = video.closest<HTMLElement>("[data-subtitle-size]");
    return [styled?.dataset["subtitleSize"], styled?.dataset["subtitleBackground"]];
  };

  await expect
    .poll(appearance)
    .toStrictEqual([String(SubtitleSize.LARGEST), String(SubtitleBackground.BOX)]);
});
