import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LibraryKind, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer } from "@/test-server";
import { openLibraries } from "@/test-settings";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type Library = MessageInitShape<typeof LibrarySchema>;

const NEWEST = "/media/artwork/tmdb/%2Falien-backdrop.jpg";
const CUSTOM = "/media/custom-images/library/1/backdrop/v1";

test("an uploaded Backdrop goes to the Library's path and shows without a reload", async () => {
  const library: Library = { id: 1n, name: "Films", kind: LibraryKind.MOVIES, backdropUrl: NEWEST };
  const { server, transport } = fakeServer({ signedIn: admin, libraries: [library] });
  // oxlint-disable-next-line require-await, typescript/require-await -- fetch is async, and this fake answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    server.libraries = [{ ...library, backdropUrl: CUSTOM, customBackdrop: true }];
    return new Response(null, { status: 204 });
  });
  openLibraries(transport, "1");
  const backdrop = await screen.findByRole("img", { name: "Backdrop" });
  expect(backdrop.getAttribute("src")).toBe(NEWEST);
  expect(screen.queryByRole("button", { name: /Remove custom image/u })).toBeNull();

  const file = new File(["backdrop"], "backdrop.jpg", { type: "image/jpeg" });
  fireEvent.change(screen.getByLabelText("Upload a Backdrop"), { target: { files: [file] } });

  await waitFor(() => {
    expect(screen.getByRole("img", { name: "Backdrop" }).getAttribute("src")).toBe(CUSTOM);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/library/1/backdrop", {
    method: "PUT",
    body: file,
  });
});

test("removing a Library's Custom Image brings the newest title's Backdrop back", async () => {
  const library: Library = {
    id: 1n,
    name: "Films",
    kind: LibraryKind.MOVIES,
    backdropUrl: CUSTOM,
    customBackdrop: true,
  };
  const { server, transport } = fakeServer({
    signedIn: [Permission.MANAGE_LIBRARIES],
    libraries: [library],
  });
  // oxlint-disable-next-line require-await, typescript/require-await -- fetch is async, and this fake answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    server.libraries = [{ ...library, backdropUrl: NEWEST, customBackdrop: false }];
    return new Response(null, { status: 204 });
  });
  openLibraries(transport, "1");

  fireEvent.click(await screen.findByRole("button", { name: "Remove custom image: Backdrop" }));

  await waitFor(() => {
    expect(screen.getByRole("img", { name: "Backdrop" }).getAttribute("src")).toBe(NEWEST);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/library/1/backdrop", { method: "DELETE" });
});
