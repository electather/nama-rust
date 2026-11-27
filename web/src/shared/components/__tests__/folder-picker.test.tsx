import { createRouterTransport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { FolderPicker } from "@/shared/components/folder-picker";
import { type Disk, listFoldersOf } from "@/test-server-folders";

afterEach(cleanup);

const disk: Disk = {
  "": ["/media", "/srv"],
  "/media": ["/media/Films", "/media/shows"],
  "/media/Films": [],
  "/media/shows": ["/media/shows/Dark"],
  "/srv": "unreadable",
};

/** Opens a picker for a field holding `path`. Returns what it asks the Server and hands back. */
function openPicker(path: string) {
  const asked: string[] = [];
  const picked: string[] = [];
  const transport = createRouterTransport((router) => {
    router.service(ServerService, { listServerFolders: listFoldersOf(disk, asked) });
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <TransportProvider transport={transport}>
      <QueryClientProvider client={client}>
        <FolderPicker
          path={path}
          onPick={(folder) => {
            picked.push(folder);
          }}
        />
      </QueryClientProvider>
    </TransportProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Pick a folder on the Server" }));
  return { asked, picked };
}

async function row(name: string) {
  const found = await screen.findByRole("treeitem", { name });
  return found;
}

/** Whether the row named `name` is open, once it's shown. */
async function isOpen(name: string) {
  const shown = await row(name);
  return shown.getAttribute("aria-expanded");
}

/** Clicks the chevron of the row named `name`, which opens or closes it. */
async function clickChevron(name: string) {
  const shown = await row(name);
  fireEvent.click(shown.querySelector("[data-slot=tree-chevron]") ?? shown);
}

function press(key: string) {
  fireEvent.keyDown(document.activeElement ?? document.body, { key });
}

function focusedName() {
  return document.activeElement?.textContent;
}

async function expectClosed() {
  await waitFor(() => {
    expect(screen.queryByRole("tree")).toBeNull();
  });
}

describe("the folder picker", () => {
  test("opens at the field's folder, every folder above it open and it selected, asking for each level once", async () => {
    const { asked } = openPicker("/media/shows");

    await expect(row("Dark")).resolves.toBeDefined();
    const shows = await row("shows");
    expect(shows.getAttribute("aria-selected")).toBe("true");
    expect(shows.getAttribute("aria-expanded")).toBe("true");
    await expect(isOpen("media")).resolves.toBe("true");
    await expect(isOpen("srv")).resolves.toBe("false");
    expect(new Set(asked)).toStrictEqual(new Set(["", "/media", "/media/shows"]));
    expect(asked).toHaveLength(3);
  });

  test("past a folder that isn't there, opens down to the last one that is", async () => {
    openPicker("/media/gone/deeper");

    const media = await row("media");
    await waitFor(() => {
      expect(media.getAttribute("aria-selected")).toBe("true");
    });
    expect(media.getAttribute("aria-expanded")).toBe("true");
    expect(screen.queryByText("This folder isn’t on the Server anymore.")).toBeNull();
  });

  test("an empty folder says it has none, and one the Server can't read says why", async () => {
    openPicker("/srv");
    await expect(screen.findByText("The Server can’t read this folder.")).resolves.toBeDefined();

    await clickChevron("media");
    await clickChevron("Films");

    await expect(screen.findByText("No folders")).resolves.toBeDefined();
  });

  test("the keyboard moves, opens, closes and picks as in a tree", async () => {
    const { picked } = openPicker("");
    const media = await row("media");
    media.focus();

    press("ArrowRight");
    await row("Films");
    await expect(isOpen("media")).resolves.toBe("true");
    press("ArrowRight");
    expect(focusedName()).toBe("Films");
    press("ArrowDown");
    expect(focusedName()).toBe("shows");
    press("ArrowLeft");
    expect(focusedName()).toBe("media");
    press("ArrowLeft");
    await expect(isOpen("media")).resolves.toBe("false");
    press("End");
    expect(focusedName()).toBe("srv");
    press("Home");
    expect(focusedName()).toBe("media");
    press("ArrowDown");
    press("ArrowUp");
    press("Enter");

    expect(picked).toStrictEqual(["/media"]);
    await expectClosed();
  });

  test("a click on a folder's name picks it, and Escape closes without picking", async () => {
    const { picked } = openPicker("/media");
    fireEvent.click(await row("Films"));

    expect(picked).toStrictEqual(["/media/Films"]);
    await expectClosed();

    fireEvent.click(screen.getByRole("button", { name: "Pick a folder on the Server" }));
    const media = await row("media");
    media.focus();
    press("Escape");

    await expectClosed();
    expect(picked).toStrictEqual(["/media/Films"]);
  });
});
