import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { KernelHomeRow } from "@/gen/nama/v1/browse_pb";
import { PluginService } from "@/gen/nama/v1/plugin_pb";
import {
  type HomeLayoutRowSchema,
  HomeLayoutSchema,
  type SetDefaultHomeLayoutRequest,
} from "@/gen/nama/v1/server_pb";
import { admin, fakeServer, newUser, openApp } from "@/test-app";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

const continueWatching = {
  row: { case: "kernel", value: KernelHomeRow.CONTINUE_WATCHING },
} as const satisfies MessageInitShape<typeof HomeLayoutRowSchema>;
const picks = {
  row: { case: "plugin", value: { pluginId: "top-picks", key: "picks", heading: "Top picks" } },
} as const satisfies MessageInitShape<typeof HomeLayoutRowSchema>;
const usersRows = {
  row: { case: "pluginUsersRows", value: "top-picks" },
} as const satisfies MessageInitShape<typeof HomeLayoutRowSchema>;

/**
 * A fake Server keeping the default Home layout, from `layout`, for an Admin: it records each save,
 * and refuses them while `refusing`.
 */
function defaultHomeServer(layout: MessageInitShape<typeof HomeLayoutSchema>, refusing = false) {
  // The Server always sets Featured in a response.
  let kept = create(HomeLayoutSchema, { featuredHidden: false, ...layout });
  const saves: SetDefaultHomeLayoutRequest[] = [];
  const transport = fakeServer(
    {
      signedIn: admin,
      serverService: {
        getDefaultHomeLayout: () => ({ layout: kept }),
        setDefaultHomeLayout: (request) => {
          saves.push(request);
          if (refusing) {
            throw new ConnectError("disk full", Code.Internal);
          }
          kept = create(HomeLayoutSchema, request.layout);
          return { layout: kept };
        },
      },
    },
    {},
    {},
    (router) => {
      router.service(PluginService, {
        listPlugins: () => ({ summaries: [{ id: "top-picks", name: "Top Picks" }] }),
      });
    },
  );
  return { transport, saves };
}

/** Opens Settings on Default Home with the default Home layout `layout`, and returns what it saves. */
function openDefaultHome(layout: MessageInitShape<typeof HomeLayoutSchema>, refusing = false) {
  const { transport, saves } = defaultHomeServer(layout, refusing);
  openSettings("default-home", transport);
  return saves;
}

/** The names of the rows the pane lists, top to bottom, under Featured. */
function rowNames() {
  return within(screen.getByRole("list"))
    .getAllByRole("listitem")
    .map((row) => row.querySelector("p")?.textContent);
}

/** The grip the row called `name` is dragged by. */
function dragHandle(name: string) {
  const handle = screen.getByText(name).closest("li")?.querySelector("span[aria-hidden]");
  if (handle === null || handle === undefined) {
    throw new Error(`${name} has no drag handle`);
  }
  return handle;
}

test("an Admin's each move and switch of a default Home row is saved as it's made", async () => {
  const saves = openDefaultHome({ rows: [continueWatching, picks, usersRows] });

  await screen.findByText("Continue Watching");
  expect(rowNames()).toStrictEqual(["Continue Watching", "Top picks", "Each User’s own rows"]);
  expect(screen.getAllByText("From Top Picks")).toHaveLength(2);

  fireEvent.click(screen.getByRole("button", { name: "Move Top picks up" }));
  fireEvent.click(screen.getByRole("switch", { name: "Show Featured" }));
  fireEvent.click(screen.getByRole("switch", { name: "Show Each User’s own rows" }));

  expect(rowNames()).toStrictEqual(["Top picks", "Continue Watching", "Each User’s own rows"]);
  expect(screen.getByText("Top picks moved to place 1 of 3.")).toBeDefined();
  expect(screen.queryByRole("button", { name: /Save/u })).toBeNull();
  await waitFor(() => {
    expect(saves).toHaveLength(3);
  });
  const reordered = [
    { ...picks, hidden: false },
    { ...continueWatching, hidden: false },
  ];
  expect(saves).toMatchObject([
    { layout: { featuredHidden: false, rows: [...reordered, { ...usersRows, hidden: false }] } },
    { layout: { featuredHidden: true, rows: [...reordered, { ...usersRows, hidden: false }] } },
    { layout: { featuredHidden: true, rows: [...reordered, { ...usersRows, hidden: true }] } },
  ]);
});

test("a change the Server refuses goes back, and says why", async () => {
  openDefaultHome({ rows: [continueWatching, picks] }, true);

  fireEvent.click(await screen.findByRole("button", { name: "Move Top picks up" }));

  await expect(screen.findByText(/Couldn’t save the default Home/u)).resolves.toBeDefined();
  expect(rowNames()).toStrictEqual(["Continue Watching", "Top picks"]);
});

test("the first row can't move up nor the last down", async () => {
  openDefaultHome({ rows: [continueWatching, picks] });

  await screen.findByText("Continue Watching");

  expect(screen.getByRole("button", { name: "Move Continue Watching up" })).toHaveProperty(
    "disabled",
    true,
  );
  expect(screen.getByRole("button", { name: "Move Top picks down" })).toHaveProperty(
    "disabled",
    true,
  );
});

test("an Admin drags a Home row to the end", async () => {
  const saves = openDefaultHome({ rows: [continueWatching, picks, usersRows] });

  await screen.findByText("Continue Watching");
  const items = within(screen.getByRole("list")).getAllByRole("listitem");
  for (const [index, item] of items.entries()) {
    vi.spyOn(item, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ y: index * 60, height: 60 }),
    );
  }
  const handle = dragHandle("Continue Watching");
  fireEvent.pointerDown(handle, { pointerId: 1, button: 0 });
  fireEvent.pointerMove(handle, { pointerId: 1, clientY: 170 });
  fireEvent.pointerUp(handle, { pointerId: 1 });

  expect(rowNames()).toStrictEqual(["Top picks", "Each User’s own rows", "Continue Watching"]);
  expect(screen.getByText("Continue Watching moved to place 3 of 3.")).toBeDefined();
  await waitFor(() => {
    expect(saves).toHaveLength(1);
  });
});

test("the old default Home address lands on Home with Settings open on Default Home", async () => {
  const router = openApp("/admin/home", defaultHomeServer({ rows: [continueWatching] }).transport);

  await screen.findByText("Continue Watching");
  expect(router.state.location.pathname).toBe("/");
  expect(router.state.location.search).toMatchObject({ settings: "default-home" });
});

test("a User who doesn't manage settings has no Default Home section", async () => {
  openSettings("default-home", fakeServer({ signedIn: newUser }));

  await screen.findByRole("heading", { name: "Playback" });
  const sections = within(screen.getByRole("navigation", { name: "Settings" }));
  expect(sections.queryByRole("link", { name: "Default Home" })).toBeNull();
});
