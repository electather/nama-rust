import type { MessageInitShape } from "@bufbuild/protobuf";
import { fireEvent, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vite-plus/test";

import { KernelHomeRow } from "@/gen/nama/v1/browse_pb";
import { PluginService } from "@/gen/nama/v1/plugin_pb";
import type {
  HomeLayoutRowSchema,
  HomeLayoutSchema,
  SetDefaultHomeLayoutRequest,
} from "@/gen/nama/v1/server_pb";
import { admin, fakeServer, openApp } from "@/test-app";

const continueWatching = {
  row: { case: "kernel", value: KernelHomeRow.CONTINUE_WATCHING },
} as const satisfies MessageInitShape<typeof HomeLayoutRowSchema>;
const picks = {
  row: { case: "plugin", value: { pluginId: "top-picks", key: "picks", heading: "Top picks" } },
} as const satisfies MessageInitShape<typeof HomeLayoutRowSchema>;
const usersRows = {
  row: { case: "pluginUsersRows", value: "top-picks" },
} as const satisfies MessageInitShape<typeof HomeLayoutRowSchema>;

/** Opens the Home page with the default Home layout `layout`, and returns what it saves. */
function openHomeLayout(layout: MessageInitShape<typeof HomeLayoutSchema>) {
  const saves: SetDefaultHomeLayoutRequest[] = [];
  openApp(
    "/admin/home",
    fakeServer(
      {
        signedIn: admin,
        serverService: {
          getDefaultHomeLayout: () => ({ layout }),
          setDefaultHomeLayout: (request) => {
            saves.push(request);
            return { layout: request.layout };
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
    ),
  );
  return saves;
}

/** The names of the rows the page lists, top to bottom, under Featured. */
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

test("an Admin reorders Home rows, hides one and Featured, and saves the default Home layout", async () => {
  const saves = openHomeLayout({ rows: [continueWatching, picks, usersRows] });

  await screen.findByText("Continue Watching");
  expect(rowNames()).toStrictEqual(["Continue Watching", "Top picks", "Each User’s own rows"]);
  expect(screen.getAllByText("From Top Picks")).toHaveLength(2);

  fireEvent.click(screen.getByRole("button", { name: "Move Top picks up" }));
  fireEvent.click(screen.getByRole("switch", { name: "Show Featured" }));
  fireEvent.click(screen.getByRole("switch", { name: "Show Each User’s own rows" }));

  expect(rowNames()).toStrictEqual(["Top picks", "Continue Watching", "Each User’s own rows"]);
  expect(screen.getByText("Top picks moved to place 1 of 3.")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Save Home layout" }));

  await expect(screen.findByText("Saved.")).resolves.toBeDefined();
  expect(saves).toMatchObject([
    {
      layout: {
        featuredHidden: true,
        rows: [
          { ...picks, hidden: false },
          { ...continueWatching, hidden: false },
          { ...usersRows, hidden: true },
        ],
      },
    },
  ]);
});

test("the first row can't move up nor the last down", async () => {
  openHomeLayout({ rows: [continueWatching, picks] });

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
  openHomeLayout({ rows: [continueWatching, picks, usersRows] });

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
});
