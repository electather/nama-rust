import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import {
  type FoundSubtitle,
  PlaybackMethod,
  PlaybackService,
  SubtitleFormat,
} from "@/gen/nama/v1/playback_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { openTracks, rowIn, rowsOf } from "@/test-player";

afterEach(() => {
  cleanup();
});

const PLAYER = "/libraries/1/movies/tmdb-348/play";
const subtitle = { case: "subtitle", value: {} } as const;
const english = {
  track: { codec: "subrip", language: "eng", title: "Full", kind: subtitle },
  place: 2,
  url: "/media/versions/7/subtitles/2",
  format: SubtitleFormat.WEBVTT,
};
const found = [
  { pluginId: "opensubtitles", id: "1", language: "fr", name: "Alien.1979.FRENCH", sdh: false },
  { pluginId: "opensubtitles", id: "2", language: "de", name: "Alien.1979.German", sdh: true },
  { pluginId: "opensubtitles", id: "3", language: "fr", name: "Alien.1979.FR.SDH", sdh: true },
];

/** How the fake Server's subtitle source answers: what it finds, or how it fails. */
interface Source {
  search?: () => { subtitles: typeof found };
  download?: () => object;
}

/**
 * A fake Server with Alien in Library 1, playing with English subtitles, for a User with
 * `permissions` whose Subtitle Preference is French. A download adds its Track to the next plan.
 */
function server(permissions: Permission[], source: Source = {}) {
  const calls: { plans: number; searched: string[][]; downloaded: FoundSubtitle[] } = {
    plans: 0,
    searched: [],
    downloaded: [],
  };
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const transport = fakeServer({ signedIn: permissions, libraries }, {}, {}, (router) => {
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
      getTrackPreferences: () => ({ subtitle: { language: "fr" } }),
      getSubtitleAppearance: () => ({}),
      getAutoplay: () => ({ on: true }),
      planPlayback: () => {
        calls.plans += 1;
        const downloads = calls.downloaded.map(({ language, name }, at) => ({
          track: { codec: "subrip", language, title: name, kind: subtitle },
          place: 3 + at,
          url: `/media/versions/7/subtitles/${3 + at}`,
          format: SubtitleFormat.WEBVTT,
        }));
        const subtitles = [english, ...downloads];
        const plan = { method: PlaybackMethod.DIRECT_PLAY, url: "/media/versions/7", subtitles };
        return { plan: { ...plan, startingSubtitle: calls.plans === 1 ? 2 : undefined } };
      },
      searchSubtitles: ({ languages }) => {
        calls.searched.push(languages);
        return source.search?.() ?? { subtitles: found };
      },
      downloadSubtitle: ({ subtitle: picked }) => {
        const answer = source.download?.() ?? {};
        if (picked) {
          calls.downloaded.push(picked);
        }
        return answer;
      },
    });
  });
  return { transport, calls };
}

const mayDownload = [...newUser, Permission.DOWNLOAD_SUBTITLES];

/** Opens the player with `permissions`, and the Audio and subtitles panel in it. */
async function openPanel(permissions: Permission[], source: Source = {}) {
  const { transport, calls } = server(permissions, source);
  openApp(PLAYER, transport);
  await screen.findByLabelText("Alien");
  openTracks();
  await screen.findByRole("radiogroup", { name: "Subtitles" });
  return calls;
}

function search() {
  fireEvent.click(screen.getByRole("button", { name: "Search for subtitles" }));
}

/** What the search found, each result's text, once it's listed. */
async function results() {
  const list = await screen.findByRole("list", { name: "Found subtitles" });
  return within(list)
    .getAllByRole("button")
    .map((button) => button.textContent);
}

describe("searching for subtitles from the player", () => {
  test("a User without Download subtitles has no search", async () => {
    await openPanel(newUser);

    expect(screen.queryByRole("button", { name: "Search for subtitles" })).toBeNull();
  });

  test("lists what's found in the Subtitle Preference's language first, marking SDH", async () => {
    const calls = await openPanel(mayDownload);

    search();

    await expect(results()).resolves.toStrictEqual([
      "French Alien.1979.FRENCH",
      "French Alien.1979.FR.SDH · SDH",
      "German Alien.1979.German · SDH",
    ]);
    expect(calls.searched).toStrictEqual([["fr", "en"]]);
  });

  test("picking one downloads it and shows its new Track", async () => {
    const calls = await openPanel(mayDownload);
    search();
    await results();

    fireEvent.click(screen.getByRole("button", { name: /FR\.SDH/u }));

    await waitFor(() => {
      expect(calls.plans).toBe(2);
    });
    expect(calls.downloaded.map(({ id }) => id)).toStrictEqual(["3"]);
    await screen.findByLabelText("Alien");
    openTracks();
    const picked = await waitFor(() => rowIn("Subtitles", /FR\.SDH/u));
    expect(picked.getAttribute("aria-checked")).toBe("true");
  });

  test("a failed search says so quietly and leaves the Track playing", async () => {
    const calls = await openPanel(mayDownload, {
      search: () => {
        throw new ConnectError("no subtitle source is enabled", Code.Unavailable);
      },
    });

    search();

    await screen.findByText("No subtitle source is enabled.");
    expect(rowIn("Subtitles", /English/u).getAttribute("aria-checked")).toBe("true");
    expect(calls.plans).toBe(1);
  });

  test("a failed download says so quietly and leaves the Track playing", async () => {
    const calls = await openPanel(mayDownload, {
      download: () => {
        throw new ConnectError("download quota reached", Code.Unavailable);
      },
    });
    search();
    await results();

    fireEvent.click(screen.getByRole("button", { name: /FRENCH/u }));

    await screen.findByText("Download quota reached.");
    expect(rowsOf("Subtitles")).toHaveLength(2);
    expect(rowIn("Subtitles", /English/u).getAttribute("aria-checked")).toBe("true");
    expect(calls.plans).toBe(1);
  });
});
