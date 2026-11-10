import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";

import { fakeServer, newUser, openApp } from "./test-app";

/** The playlist each hls.js player loaded. */
const hlsJs = vi.hoisted(() => ({ played: [] as string[] }));
vi.mock("hls.js", () => ({
  default: class {
    static Events = { ERROR: "hlsError" };
    url = "";
    on() {}
    loadSource(url: string) {
      this.url = url;
    }
    attachMedia() {
      hlsJs.played.push(this.url);
    }
    destroy() {}
  },
}));

afterEach(() => {
  cleanup();
  hlsJs.played = [];
  vi.restoreAllMocks();
});

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const ENGLISH = "/media/versions/7/subtitles/2";
const FRENCH = "/media/versions/7/subtitles/3";
const FILE = "/media/versions/7";
const PLAYLIST = "/media/plans/0/index.m3u8";

interface Plan {
  method: PlaybackMethod;
  url: string;
}
const directPlay: Plan = { method: PlaybackMethod.DIRECT_PLAY, url: FILE };
const remux: Plan = { method: PlaybackMethod.REMUX, url: PLAYLIST };

/**
 * The ways a Version plays: its plan, what the browser says to `canPlayType` for HLS ("" for no
 * HLS of its own), and what the `<video>` plays, as `playing` gives it.
 */
const ways = [
  { how: "Direct Play", plan: directPlay, canPlayHls: "", plays: { src: FILE, hlsJs: [] } },
  {
    how: "a Remux through hls.js",
    plan: remux,
    canPlayHls: "",
    plays: { src: null, hlsJs: [PLAYLIST] },
  },
  {
    how: "a Remux played natively",
    plan: remux,
    canPlayHls: "maybe",
    plays: { src: PLAYLIST, hlsJs: [] },
  },
] as const;

/** What `video` is playing: its `src`, and the playlists hls.js loaded into it. */
function playing(video: HTMLVideoElement) {
  return { src: video.getAttribute("src"), hlsJs: hlsJs.played };
}

/**
 * A fake Server with Alien in Library 1, whose `plan` offers English SDH and French Forced
 * subtitles.
 */
function server(plan = directPlay) {
  const plans = { asked: 0 };
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const subtitle = { case: "subtitle", value: {} } as const;
  const transport = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: {
          id: { id: { case: "tmdb", value: 348n } },
          title: "Alien",
          versions: [{ id: 7n, fileName: "Alien (1979).mkv", size: 1n }],
        },
      }),
    });
    router.service(PlaybackService, {
      planPlayback: () => {
        plans.asked += 1;
        return {
          plan: {
            ...plan,
            subtitles: [
              {
                track: {
                  codec: "subrip",
                  language: "eng",
                  title: "Full",
                  sdh: true,
                  kind: subtitle,
                },
                url: ENGLISH,
              },
              {
                track: { codec: "subrip", language: "fre", forced: true, kind: subtitle },
                url: FRENCH,
              },
            ],
          },
        };
      },
    });
  });
  return { transport, plans };
}

/**
 * The `<video>` and, by URL, a stand-in for each of its `<track>`s' text tracks: happy-dom makes a
 * new one each time it's asked, so a mode set on one is lost.
 */
async function player() {
  const video = await screen.findByLabelText<HTMLVideoElement>("Alien");
  const modes = new Map<string, { mode: TextTrackMode }>();
  for (const element of video.querySelectorAll("track")) {
    const track = { mode: "disabled" as TextTrackMode };
    Object.defineProperty(element, "track", { value: track });
    modes.set(element.getAttribute("src") ?? "", track);
  }
  return { video, modes };
}

function choose(name: RegExp) {
  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  fireEvent.click(screen.getByRole("menuitemradio", { name }));
}

function loaded(video: HTMLVideoElement, url: string) {
  fireEvent.load(video.querySelector(`track[src="${url}"]`) ?? video);
}

test("the picker lists the text subtitles by language and title, with Forced and SDH", async () => {
  openApp(PLAYER, server().transport);
  await player();

  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));

  const items = await screen.findAllByRole("menuitemradio");
  expect(items.map((item) => item.textContent)).toEqual([
    "Off",
    "English Full · SDH",
    "French Forced",
  ]);
  expect(screen.getByRole("menuitemradio", { name: "Off" }).getAttribute("aria-checked")).toBe(
    "true",
  );
});

test("a subtitle turned on says it's loading until it has loaded", async () => {
  openApp(PLAYER, server().transport);
  const { video, modes } = await player();

  choose(/English/u);

  const english = await screen.findByRole("menuitemradio", { name: /English/u });
  expect(english.textContent).toContain("Loading…");
  expect(modes.get(ENGLISH)?.mode).toBe("showing");
  loaded(video, ENGLISH);
  await waitFor(() => {
    expect(english.textContent).not.toContain("Loading…");
  });
  expect(english.getAttribute("aria-checked")).toBe("true");
});

test.each(ways)(
  "switching subtitles in $how neither asks for a new plan nor reloads the video",
  async (way) => {
    vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockReturnValue(way.canPlayHls);
    const { transport, plans } = server(way.plan);
    openApp(PLAYER, transport);
    const { video, modes } = await player();
    choose(/English/u);
    loaded(video, ENGLISH);

    fireEvent.click(screen.getByRole("menuitemradio", { name: /French/u }));

    await waitFor(() => {
      expect(modes.get(FRENCH)?.mode).toBe("showing");
    });
    expect(modes.get(ENGLISH)?.mode).toBe("disabled");
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Off" }));
    await waitFor(() => {
      expect(modes.get(FRENCH)?.mode).toBe("disabled");
    });
    expect(plans.asked).toBe(1);
    expect(screen.getByLabelText("Alien")).toBe(video);
    expect(playing(video)).toEqual(way.plays);
  },
);

test("Space in the subtitle menu picks a subtitle without playing or pausing", async () => {
  openApp(PLAYER, server().transport);
  const { video, modes } = await player();
  const [play, pause] = [vi.spyOn(video, "play"), vi.spyOn(video, "pause")];
  fireEvent.click(screen.getByRole("button", { name: "Subtitles" }));
  const french = await screen.findByRole("menuitemradio", { name: /French/u });

  fireEvent.keyDown(french, { key: " " });

  await waitFor(() => {
    expect(modes.get(FRENCH)?.mode).toBe("showing");
  });
  expect(play).not.toHaveBeenCalled();
  expect(pause).not.toHaveBeenCalled();
});
