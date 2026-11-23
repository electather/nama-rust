import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { PlaybackMethod, PlaybackService } from "@/gen/nama/v1/playback_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { findSettingsLink } from "@/test-settings";

afterEach(cleanup);

// A test's first wait covers the route's chunks loading, which takes over `expect.poll`'s 1 s on a
// busy CI runner.
const loaded = { timeout: 5000 };

/** A fake Server whose Movies Library, Films (1), has The Matrix (tmdb-603). */
function server() {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listGenres: () => ({}),
      listUnmatched: () => ({}),
      listMovies: () => ({ movies: [] }),
      getMovie: () => {
        const versions = [{ id: 7n, fileName: "Matrix.mkv", size: 1n }];
        return {
          movie: { id: { namespace: "tmdb", value: "603" }, title: "The Matrix", versions },
        };
      },
    });
    router.service(PlaybackService, {
      planPlayback: () => ({ plan: { id: 1n, method: PlaybackMethod.DIRECT_PLAY, url: "/v/7" } }),
    });
  });
}

test("a Library's tab names it, and a Movie's names it once the app goes there", async () => {
  const router = openApp("/libraries/1", server());
  await expect.poll(() => document.title, loaded).toBe("Films – Nama");

  await act(async () => {
    await router.navigate({
      to: "/libraries/$libraryId/movies/$movieId",
      params: { libraryId: "1", movieId: "tmdb-603" },
    });
  });

  await expect.poll(() => document.title).toBe("The Matrix – Nama");
});

test("the player's tab names what it plays", async () => {
  openApp("/libraries/1/movies/tmdb-603/play", server());

  await expect.poll(() => document.title, loaded).toBe("Playing The Matrix – Nama");
});

test("settings' tab names its section, follows it to another, and gives way to the page as it closes", async () => {
  openApp("/libraries/1?settings=playback", server());
  await expect.poll(() => document.title, loaded).toBe("Settings: Playback – Nama");

  fireEvent.click(await findSettingsLink("Home"));
  await expect.poll(() => document.title).toBe("Settings: Home – Nama");

  fireEvent.click(screen.getByRole("button", { name: "Close Settings" }));
  await expect.poll(() => document.title).toBe("Films – Nama");
});

test("an address with nothing at it says so in the tab", async () => {
  openApp("/nothing-here", server());

  await expect.poll(() => document.title, loaded).toBe("Page not found – Nama");
});

test("the sign-in page's tab names the Server", async () => {
  openApp("/sign-in", fakeServer({}));

  await expect.poll(() => document.title, loaded).toBe("Sign in to Living Room – Nama");
});
