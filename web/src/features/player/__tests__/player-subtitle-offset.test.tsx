import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  PlaybackMethod,
  PlaybackService,
  type SetSubtitleOffsetRequest,
  SubtitleFormat,
} from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { openTracks, pickIn } from "@/test-player";

afterEach(cleanup);

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const ENGLISH = "/media/versions/7/subtitles/2";
const FRENCH = "/media/versions/7/subtitles/3";
const SUBTITLE = { case: "subtitle", value: {} } as const;

/**
 * A fake Server with Alien, whose plan offers English, which the User set 1.5 s later, and French,
 * as `<track>`s, or else burns in the Track at `burnedIn`. Each offset saved goes in `saved`.
 */
function server(saved: SetSubtitleOffsetRequest[], burnedIn?: number) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { namespace: "tmdb", value: "348" },
          title: "Alien",
          versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n }],
        },
      }),
    });
    router.service(PlaybackService, {
      planPlayback: () => ({
        plan: {
          method: PlaybackMethod.DIRECT_PLAY,
          url: "/media/versions/7",
          subtitles: [
            {
              track: { codec: "subrip", language: "eng", kind: SUBTITLE },
              url: ENGLISH,
              place: 2,
              format: SubtitleFormat.WEBVTT,
              offsetMs: 1500,
            },
            {
              track: { codec: "subrip", language: "fre", kind: SUBTITLE },
              url: FRENCH,
              place: 3,
              format: SubtitleFormat.WEBVTT,
            },
          ],
          burnInTracks: burnedIn === undefined ? [] : [burnedIn],
          subtitleTrack: burnedIn,
          startingSubtitle: burnedIn,
        },
      }),
      setSubtitleOffset: (request) => {
        saved.push(request);
        return {};
      },
    });
  });
}

/**
 * Opens the player and gives each `<track>` one cue, from 10 s to 12 s as its file times it, in a
 * stand-in for its text track, which happy-dom makes anew each time it's asked. As a browser's, it
 * has no cues while disabled. Returns, by URL, when each cue starts and ends now.
 */
async function playing(saved: SetSubtitleOffsetRequest[], burnedIn?: number) {
  openApp(PLAYER, server(saved, burnedIn));
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  const cues = new Map<string, VTTCue>();
  for (const element of video.querySelectorAll("track")) {
    const cue = new VTTCue(10, 12, "Hello");
    const track = {
      mode: "disabled",
      get cues() {
        return this.mode === "disabled" ? null : [cue];
      },
    };
    Object.defineProperty(element, "track", { value: track });
    cues.set(element.getAttribute("src") ?? "", cue);
  }
  const loaded = (url: string) => {
    fireEvent.load(video.querySelector(`track[src="${url}"]`) ?? video);
  };
  const timed = (url: string) => {
    const cue = cues.get(url);
    return [tenths(cue?.startTime), tenths(cue?.endTime)];
  };
  return { loaded, timed };
}

/** `seconds` to the tenth, as adding steps drifts. */
function tenths(seconds = Number.NaN) {
  return Math.round(seconds * 10) / 10;
}

function offset() {
  return within(screen.getByRole("group", { name: "Offset" }));
}

function shift(by: "Show earlier" | "Show later") {
  fireEvent.click(offset().getByRole("button", { name: by }));
}

test("a text subtitle's cues show its Track's offset once loaded, and a nudge shifts and saves it", async () => {
  const saved: SetSubtitleOffsetRequest[] = [];
  const { loaded, timed } = await playing(saved);

  pickIn("Subtitles", /English/u);
  loaded(ENGLISH);

  await waitFor(() => {
    expect(timed(ENGLISH)).toStrictEqual([11.5, 13.5]);
  });
  expect(offset().getByRole("status").textContent).toBe("+1.5 s");
  shift("Show earlier");
  shift("Show earlier");
  await waitFor(() => {
    expect(timed(ENGLISH)).toStrictEqual([11.3, 13.3]);
  });
  expect(offset().getByRole("status").textContent).toBe("+1.3 s");
  await waitFor(() => {
    expect(
      saved.map(({ versionId, track, offsetMs }) => [versionId, track, offsetMs]),
    ).toStrictEqual([
      [7n, 2, 1400],
      [7n, 2, 1300],
    ]);
  });
});

test("each Track keeps its own offset, so switching away and back restores it", async () => {
  const { loaded, timed } = await playing([]);
  pickIn("Subtitles", /English/u);
  loaded(ENGLISH);
  shift("Show later");

  pickIn("Subtitles", /French/u);
  loaded(FRENCH);
  const french = offset().getByRole("status").textContent;
  shift("Show earlier");
  pickIn("Subtitles", /English/u);

  expect(french).toBe("0.0 s");
  expect(offset().getByRole("status").textContent).toBe("+1.6 s");
  await waitFor(() => {
    expect(timed(FRENCH)).toStrictEqual([9.9, 11.9]);
  });
  expect(timed(ENGLISH)).toStrictEqual([11.6, 13.6]);
});

test("a text subtitle the browser loaded before it was picked shows its Track's offset once picked", async () => {
  const { loaded, timed } = await playing([]);
  // As Chrome loads the one the system's captions setting prefers, which the player then hides.
  loaded(ENGLISH);

  pickIn("Subtitles", /English/u);

  await waitFor(() => {
    expect(timed(ENGLISH)).toStrictEqual([11.5, 13.5]);
  });
});

test("with subtitles off there's no offset", async () => {
  await playing([]);

  openTracks();

  expect(screen.queryByRole("group", { name: "Offset" })).toBeNull();
});

test("a burned-in Track's offset is disabled, saying why", async () => {
  await playing([], 4);

  openTracks();

  const group = screen.getByRole("group", { name: "Offset" });
  expect(group.hasAttribute("disabled")).toBe(true);
  const note = within(group).getByText("Burned into the picture, so it can’t be offset.");
  expect(group.getAttribute("aria-describedby")).toBe(note.id);
});
