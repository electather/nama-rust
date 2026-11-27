import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { rgbaToThumbHash, thumbHashToDataURL } from "thumbhash";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import { admin, fakeServer, openApp } from "@/test-app";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type Library = MessageInitShape<typeof LibrarySchema>;

const NEWEST = "/media/artwork/tmdb/%2Falien-backdrop.jpg";
const CUSTOM = "/media/custom-images/library/1/backdrop/v1";
const thumbhash = rgbaToThumbHash(1, 1, new Uint8Array(4));

/** A fake Server whose one Library is `library()`, as the Server has it now, holding no titles. */
function server(library: () => Library) {
  return fakeServer(
    {
      signedIn: admin,
      libraryService: {
        listLibraries: () => ({ libraries: [library()] }),
        getLibrary: () => ({ library: library() }),
      },
    },
    {},
    {},
    (router) => {
      router.service(BrowseService, {
        listUnmatched: () => ({}),
        listGenres: () => ({}),
        listMovies: () => ({ page: { totalSize: 0 } }),
      });
    },
  );
}

/** The Library page's `<img>` drawing `src`, if any. */
function image(src: string) {
  return document.querySelector(`img[src="${src}"]`);
}

test("a Library with a Backdrop shows it behind its name, under its ThumbHash until it loads", async () => {
  const films = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, backdropUrl: NEWEST };
  openApp(
    "/libraries/1",
    server(() => ({ ...films, backdropThumbhash: thumbhash })),
  );

  const heading = await screen.findByRole("heading", { level: 1, name: "Films" });

  const backdrop = image(NEWEST);
  expect(backdrop).not.toBeNull();
  expect(heading.closest("header")?.contains(backdrop)).toBe(true);
  expect(image(thumbHashToDataURL(thumbhash))).not.toBeNull();
});

test("a Library without a Backdrop shows its name as a plain heading", async () => {
  openApp(
    "/libraries/1",
    server(() => ({ id: 1n, name: "Films", kind: LibraryKind.MOVIES })),
  );

  const heading = await screen.findByRole("heading", { level: 1, name: "Films" });

  expect(heading.closest("header")?.querySelector("img")).toBeNull();
});

/** A Library's Backdrop fields when it shows `url`. */
function custom(url: string) {
  return { backdropUrl: url, customBackdrop: url === CUSTOM };
}

/** Each way to change the Library's Custom Image in Settings' Libraries section. */
const changes = {
  "uploading a Custom Image": () => {
    const file = new File(["backdrop"], "backdrop.jpg", { type: "image/jpeg" });
    fireEvent.change(screen.getByLabelText("Upload a Backdrop"), { target: { files: [file] } });
  },
  "removing the Custom Image": () => {
    fireEvent.click(screen.getByRole("button", { name: "Remove custom image: Backdrop" }));
  },
};

test.each([
  { name: "uploading a Custom Image", before: NEWEST, after: CUSTOM },
  { name: "removing the Custom Image", before: CUSTOM, after: NEWEST },
] as const)(
  "$name for a Library in Settings changes its page's Backdrop",
  async ({ name, before, after }) => {
    let library: Library = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, ...custom(before) };
    // oxlint-disable-next-line require-await, typescript/require-await -- fetch is async, and this fake answers at once
    vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
      library = { ...library, ...custom(after) };
      return new Response(null, { status: 204 });
    });
    openApp(
      "/libraries/1?settings=libraries&library=1",
      server(() => library),
    );

    await screen.findByLabelText("Upload a Backdrop");
    changes[name]();

    // Settings' preview draws it too, and its dialog hides the page from the accessibility tree.
    const heading = screen.getByRole("heading", { level: 1, name: "Films", hidden: true });
    await waitFor(() => {
      expect(heading.closest("header")?.querySelector(`img[src="${after}"]`)).not.toBeNull();
    });
  },
);
