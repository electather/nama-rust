import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { KernelHomeRow } from "@/gen/nama/v1/browse_pb";
import { type HomeLayout, HomeLayoutSchema } from "@/gen/nama/v1/server_pb";
import { rowKey } from "@/shared/lib/home-layout";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

/** The Server's default as this User sees it: Continue Watching, then a Plugin's row. */
function defaultLayout() {
  return create(HomeLayoutSchema, {
    featuredHidden: false,
    rows: [
      { row: { case: "kernel", value: KernelHomeRow.CONTINUE_WATCHING } },
      {
        row: {
          case: "plugin",
          value: { pluginId: "top-picks", key: "picks", heading: "Top picks" },
        },
      },
    ],
  });
}

/**
 * Opens Settings on a fake Server keeping this User's Home layout, and returns what it's asked to
 * save, and each order it's asked to reorder the rows in, which it answers once `reordered`
 * settles. Like the Server, it keeps the flags of the rows a request leaves out, and Featured's
 * when a request leaves it unset.
 */
function openSettings(reordered = Promise.resolve()) {
  let layout = defaultLayout();
  const saves: HomeLayout[] = [];
  const orders: string[][] = [];
  openApp(
    "/code?settings=home",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        getHomeLayout: () => ({ layout }),
        setHomeLayout: (request) => {
          const asked = request.layout ?? defaultLayout();
          saves.push(asked);
          const named = new Map(asked.rows.map((row) => [rowKey(row), row]));
          layout = create(HomeLayoutSchema, {
            featuredHidden: asked.featuredHidden ?? layout.featuredHidden,
            rows: layout.rows.map((row) => named.get(rowKey(row)) ?? row),
          });
          return { layout };
        },
        reorderHomeRows: async (request) => {
          orders.push(request.rows.map(rowKey));
          await reordered;
          const saved = new Map(layout.rows.map((row) => [rowKey(row), row]));
          const rows = request.rows.flatMap((row) => saved.get(rowKey(row)) ?? []);
          layout = create(HomeLayoutSchema, { ...layout, rows });
          return { layout };
        },
        resetHomeLayout: () => {
          layout = defaultLayout();
          return {};
        },
      },
    ),
  );
  return { saves, orders };
}

function rowLabels(home: HTMLElement) {
  return within(home)
    .getAllByRole("switch")
    .map((shown) => shown.getAttribute("aria-label"));
}

function shownSwitch(home: HTMLElement, name: string) {
  return within(home).getByRole("switch", { name: `Show ${name}` });
}

/** The Home pane, once its rows have loaded: until then the same frame holds a skeleton. */
async function homePane() {
  await screen.findByRole("switch", { name: "Show Featured" });
  return screen.getByRole("region", { name: "Home" });
}

test("a User hides a Home row and Featured on Settings, then resets to the default", async () => {
  const { saves } = openSettings();
  const home = await homePane();

  fireEvent.click(shownSwitch(home, "Top picks"));
  await expect.poll(() => saves.at(-1)?.rows[0]?.hidden).toBe(true);
  await expect
    .poll(() => shownSwitch(home, "Top picks").getAttribute("aria-checked"))
    .toBe("false");
  fireEvent.click(shownSwitch(home, "Featured"));
  await expect.poll(() => saves.at(-1)?.featuredHidden).toBe(true);
  expect(shownSwitch(home, "Top picks").getAttribute("aria-checked")).toBe("false");
  expect(shownSwitch(home, "Continue Watching").getAttribute("aria-checked")).toBe("true");

  fireEvent.click(within(home).getByRole("button", { name: "Reset to default" }));

  await expect.poll(() => shownSwitch(home, "Top picks").getAttribute("aria-checked")).toBe("true");
  expect(shownSwitch(home, "Featured").getAttribute("aria-checked")).toBe("true");
});

test("flipping a switch saves that row or Featured alone, so the rest follow the default", async () => {
  const { saves } = openSettings();
  const home = await homePane();

  fireEvent.click(shownSwitch(home, "Top picks"));
  await expect.poll(() => saves.length).toBe(1);
  expect(saves[0]?.rows.map(rowKey)).toStrictEqual(["plugin/top-picks/picks"]);
  expect(saves[0]?.featuredHidden).toBeUndefined();
  await expect
    .poll(() => shownSwitch(home, "Top picks").getAttribute("aria-checked"))
    .toBe("false");
  fireEvent.click(shownSwitch(home, "Featured"));
  await expect.poll(() => saves.length).toBe(2);
  expect(saves[1]?.rows).toStrictEqual([]);
  expect(saves[1]?.featuredHidden).toBe(true);
});

test("moving a row saves the User's order of their Home rows and says where it went", async () => {
  const { orders } = openSettings();
  const home = await homePane();

  fireEvent.click(await within(home).findByRole("button", { name: "Move Top picks up" }));

  await expect.poll(() => orders).toStrictEqual([["plugin/top-picks/picks", "kernel/1"]]);
  await expect
    .poll(() => rowLabels(home))
    .toStrictEqual(["Show Featured", "Show Top picks", "Show Continue Watching"]);
  expect(within(home).getByRole("status").textContent).toBe("Top picks moved to place 1 of 2.");
});

test("a switch flipped while a move saves stays as flipped", async () => {
  let answerReorder: (() => void) | undefined;
  const reordered = new Promise<void>((resolve) => {
    answerReorder = resolve;
  });
  const { saves } = openSettings(reordered);
  const home = await homePane();

  fireEvent.click(await within(home).findByRole("button", { name: "Move Top picks up" }));
  fireEvent.click(shownSwitch(home, "Top picks"));
  await expect.poll(() => saves.length).toBe(1);

  await expect
    .poll(() => shownSwitch(home, "Top picks").getAttribute("aria-checked"))
    .toBe("false");
  answerReorder?.();
  await expect
    .poll(() => rowLabels(home))
    .toStrictEqual(["Show Featured", "Show Top picks", "Show Continue Watching"]);
  expect(shownSwitch(home, "Top picks").getAttribute("aria-checked")).toBe("false");
});
