import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService, type MovieSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, type ListArtworkResponseSchema } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const managesLibraries = [...newUser, Permission.MANAGE_LIBRARIES];

const DOWNLOADED = "/media/artwork/tmdb/%2Falien.jpg";
const CUSTOM = "/media/custom-images/movie/tmdb-348/poster/v1";
const OTHER = "/media/artwork/tmdb/%2Fother.jpg";

/**
 * Alien's page for a User with `permissions`, whose Poster is whatever `movie` says now, and whose
 * title sources list the artwork `listed` to pick from.
 */
function openAlien(
  permissions: Permission[],
  movie: () => MessageInitShape<typeof MovieSchema>,
  listed: MessageInitShape<typeof ListArtworkResponseSchema> = {},
) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const libraryService = { listArtwork: () => listed };
  const transport = fakeServer(
    { signedIn: permissions, libraries, libraryService },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        getMovie: () => ({
          movie: { title: "Alien", versions: [{ id: 1n, fileName: "Alien.mkv" }], ...movie() },
        }),
      });
    },
  );
  openApp("/libraries/1/movies/tmdb-348", transport);
}

async function openEditImages() {
  fireEvent.click(await screen.findByRole("button", { name: "Edit images" }));
  return screen.findByRole("dialog", { name: "Images of Alien" });
}

/** Picks a file as the new Poster in `dialog`. */
function uploadPoster(dialog: HTMLElement) {
  const file = new File(["poster"], "poster.jpg", { type: "image/jpeg" });
  fireEvent.change(within(dialog).getByLabelText("Upload a Poster"), {
    target: { files: [file] },
  });
  return file;
}

test("only a User with Manage Libraries sees Edit images", async () => {
  openAlien(newUser, () => ({}));
  await screen.findByRole("link", { name: "Play" });
  expect(screen.queryByRole("button", { name: "Edit images" })).toBeNull();
  cleanup();

  openAlien(managesLibraries, () => ({}));

  await expect(screen.findByRole("button", { name: "Edit images" })).resolves.toBeDefined();
});

test("an uploaded Poster goes to the Movie's path and shows without a reload", async () => {
  let poster = { posterUrl: DOWNLOADED, customPoster: false };
  // oxlint-disable-next-line require-await, typescript/require-await -- a fake fetch answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    poster = { posterUrl: CUSTOM, customPoster: true };
    return new Response(null, { status: 204 });
  });
  openAlien(managesLibraries, () => poster);
  const dialog = await openEditImages();

  const file = uploadPoster(dialog);

  await waitFor(() => {
    expect(within(dialog).getByRole("img", { name: "Poster" }).getAttribute("src")).toBe(CUSTOM);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/movie/tmdb-348/poster", {
    method: "PUT",
    body: file,
  });
  expect(within(dialog).getByRole("button", { name: "Remove custom image: Poster" })).toBeDefined();
});

test("removing a Custom Image brings the downloaded one back", async () => {
  let poster = { posterUrl: CUSTOM, customPoster: true };
  // oxlint-disable-next-line require-await, typescript/require-await -- a fake fetch answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    poster = { posterUrl: DOWNLOADED, customPoster: false };
    return new Response(null, { status: 204 });
  });
  openAlien(managesLibraries, () => poster);
  const dialog = await openEditImages();

  fireEvent.click(within(dialog).getByRole("button", { name: "Remove custom image: Poster" }));

  await waitFor(() => {
    expect(within(dialog).getByRole("img", { name: "Poster" }).getAttribute("src")).toBe(
      DOWNLOADED,
    );
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/movie/tmdb-348/poster", {
    method: "DELETE",
  });
  expect(within(dialog).queryByRole("button", { name: "Remove custom image: Poster" })).toBeNull();
});

test("a Poster picked from the title sources is downloaded by the Server and shows", async () => {
  let poster = { posterUrl: DOWNLOADED, customPoster: false };
  // oxlint-disable-next-line require-await, typescript/require-await -- a fake fetch answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    poster = { posterUrl: CUSTOM, customPoster: true };
    return new Response(null, { status: 204 });
  });
  openAlien(managesLibraries, () => poster, { posterUrls: [DOWNLOADED, OTHER] });
  const dialog = await openEditImages();

  fireEvent.click(await within(dialog).findByRole("button", { name: "Pick Poster 2 of 2" }));

  await waitFor(() => {
    expect(within(dialog).getByRole("img", { name: "Poster" }).getAttribute("src")).toBe(CUSTOM);
  });
  expect(fetched).toHaveBeenCalledWith(
    `/custom-images/movie/tmdb-348/poster?artwork=${encodeURIComponent(OTHER)}`,
    { method: "PUT" },
  );
});

test("an image no title source lists artwork for offers only the upload", async () => {
  openAlien(managesLibraries, () => ({ posterUrl: DOWNLOADED }), { backdropUrls: [OTHER] });
  const dialog = await openEditImages();

  // The Backdrop's pick shows once the lists have arrived.
  await within(dialog).findByRole("button", { name: "Pick Backdrop 1 of 1" });

  expect(within(dialog).getByLabelText("Upload a Poster")).toBeDefined();
  expect(within(dialog).queryByRole("button", { name: /^Pick Poster/u })).toBeNull();
});

test.each([
  [413, "That file is over 20 MB. Pick a smaller one."],
  [415, "That isn’t a JPEG, PNG or WebP image."],
  [422, "That image is over 12 megapixels. Pick a smaller one."],
])("a refused upload (%i) says what was wrong", async (status, message) => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("refused", { status }));
  openAlien(managesLibraries, () => ({ posterUrl: DOWNLOADED }));
  const dialog = await openEditImages();

  uploadPoster(dialog);

  const alert = await within(dialog).findByRole("alert");
  expect(alert.textContent).toBe(message);
});

test("a Show's page edits the Show's images and the Season shown's Poster", async () => {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  const transport = fakeServer({ signedIn: managesLibraries, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getShow: () => ({ show: { title: "Dark" }, seasons: [{ number: 1 }, { number: 2 }] }),
      listEpisodes: () => ({ episodes: [] }),
    });
  });
  const fetched = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValue(new Response(null, { status: 204 }));
  openApp("/libraries/2/shows/tmdb-70523?season=2", transport);
  fireEvent.click(await screen.findByRole("button", { name: "Edit images" }));
  const dialog = await screen.findByRole("dialog", { name: "Images of Dark" });

  for (const label of ["Upload a Poster", "Upload a Backdrop", "Upload a Season 2 Poster"]) {
    fireEvent.change(within(dialog).getByLabelText(label), {
      target: { files: [new File(["image"], "image.png", { type: "image/png" })] },
    });
  }

  await waitFor(() => {
    expect(fetched.mock.calls.map(([path]) => path)).toStrictEqual([
      "/custom-images/show/tmdb-70523/poster",
      "/custom-images/show/tmdb-70523/backdrop",
      "/custom-images/show/tmdb-70523/season/2/poster",
    ]);
  });
});

/** Dark's page showing Season 2, whose title sources list `seasonPosters` for that Season. */
async function openDarkSeason2(seasonPosters: string[]) {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  let listedSeason: (() => void) | undefined;
  const seasonListed = new Promise<void>((resolve) => {
    listedSeason = resolve;
  });
  const libraryService = {
    // The Show's lists answer only after the Season's, so once the Show's picks show, the
    // Season's list has arrived too, empty or not.
    listArtwork: async ({ season }: { season?: number | undefined }) => {
      if (season === 2) {
        listedSeason?.();
        return { posterUrls: seasonPosters };
      }
      await seasonListed;
      return { posterUrls: [OTHER], backdropUrls: [OTHER] };
    },
  };
  const transport = fakeServer(
    { signedIn: managesLibraries, libraries, libraryService },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        getShow: () => ({ show: { title: "Dark" }, seasons: [{ number: 1 }, { number: 2 }] }),
        listEpisodes: () => ({ episodes: [] }),
      });
    },
  );
  openApp("/libraries/2/shows/tmdb-70523?season=2", transport);
  fireEvent.click(await screen.findByRole("button", { name: "Edit images" }));
  return screen.findByRole("dialog", { name: "Images of Dark" });
}

test("a Season's Poster is picked from that Season's artwork", async () => {
  const fetched = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValue(new Response(null, { status: 204 }));
  const dialog = await openDarkSeason2([DOWNLOADED, OTHER]);

  fireEvent.click(
    await within(dialog).findByRole("button", { name: "Pick Season 2 Poster 2 of 2" }),
  );

  await waitFor(() => {
    expect(fetched).toHaveBeenCalledWith(
      `/custom-images/show/tmdb-70523/season/2/poster?artwork=${encodeURIComponent(OTHER)}`,
      { method: "PUT" },
    );
  });
});

test("a Season no title source lists artwork for offers only the upload", async () => {
  const dialog = await openDarkSeason2([]);

  await within(dialog).findByRole("button", { name: "Pick Poster 1 of 1" });

  expect(within(dialog).getByLabelText("Upload a Season 2 Poster")).toBeDefined();
  expect(within(dialog).queryByRole("button", { name: /^Pick Season 2 Poster/u })).toBeNull();
});

test("a file over 20 MiB is refused before it's sent", async () => {
  const fetched = vi.spyOn(globalThis, "fetch");
  openAlien(managesLibraries, () => ({ posterUrl: DOWNLOADED }));
  const dialog = await openEditImages();
  const file = new File(["poster"], "poster.jpg", { type: "image/jpeg" });
  // A browser can lose the Server's refusal of a body it stops reading partway.
  Object.defineProperty(file, "size", { value: 20 * 1024 * 1024 + 1 });

  fireEvent.change(within(dialog).getByLabelText("Upload a Poster"), {
    target: { files: [file] },
  });

  const alert = await within(dialog).findByRole("alert");
  expect(alert.textContent).toBe("That file is over 20 MB. Pick a smaller one.");
  expect(fetched).not.toHaveBeenCalled();
});
