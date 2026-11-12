import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService, type MovieSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const managesLibraries = [...newUser, Permission.MANAGE_LIBRARIES];

const DOWNLOADED = "/media/artwork/tmdb/%2Falien.jpg";
const CUSTOM = "/media/custom-images/movie/tmdb-348/poster/v1";

/** Alien's page for a User with `permissions`, whose Poster is whatever `movie` says now. */
function openAlien(permissions: Permission[], movie: () => MessageInitShape<typeof MovieSchema>) {
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const transport = fakeServer({ signedIn: permissions, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({
        movie: { title: "Alien", versions: [{ id: 1n, fileName: "Alien.mkv" }], ...movie() },
      }),
    });
  });
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

  expect(await screen.findByRole("button", { name: "Edit images" })).toBeDefined();
});

test("an uploaded Poster goes to the Movie's path and shows without a reload", async () => {
  let poster = { posterUrl: DOWNLOADED, customPoster: false };
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(() => {
    poster = { posterUrl: CUSTOM, customPoster: true };
    return Promise.resolve(new Response(null, { status: 204 }));
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
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(() => {
    poster = { posterUrl: DOWNLOADED, customPoster: false };
    return Promise.resolve(new Response(null, { status: 204 }));
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

test.each([
  [413, "That file is over 20 MB. Pick a smaller one."],
  [415, "That isn’t a JPEG, PNG or WebP image."],
  [422, "That image is over 12 megapixels. Pick a smaller one."],
])("a refused upload (%i) says what was wrong", async (status, message) => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("refused", { status }));
  openAlien(managesLibraries, () => ({ posterUrl: DOWNLOADED }));
  const dialog = await openEditImages();

  uploadPoster(dialog);

  expect((await within(dialog).findByRole("alert")).textContent).toBe(message);
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
    expect(fetched.mock.calls.map(([path]) => path)).toEqual([
      "/custom-images/show/tmdb-70523/poster",
      "/custom-images/show/tmdb-70523/backdrop",
      "/custom-images/show/tmdb-70523/season/2/poster",
    ]);
  });
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

  expect((await within(dialog).findByRole("alert")).textContent).toBe(
    "That file is over 20 MB. Pick a smaller one.",
  );
  expect(fetched).not.toHaveBeenCalled();
});
