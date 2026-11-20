import { create } from "@bufbuild/protobuf";
import { fireEvent, screen, within } from "@testing-library/react";
import { expect, test } from "vite-plus/test";

import { KernelHomeRow } from "@/gen/nama/v1/browse_pb";
import { type HomeLayout, HomeLayoutSchema } from "@/gen/nama/v1/server_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

/** The Server's default as this User sees it: Continue Watching, then a Plugin's row. */
function defaultLayout() {
  return create(HomeLayoutSchema, {
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

/** Opens Settings on a fake Server keeping this User's Home layout, and returns what it saves. */
function openSettings() {
  let layout = defaultLayout();
  const saves: HomeLayout[] = [];
  openApp(
    "/settings",
    fakeServer(
      { signedIn: newUser },
      {},
      {
        getHomeLayout: () => ({ layout }),
        setHomeLayout: (request) => {
          layout = request.layout ?? defaultLayout();
          saves.push(layout);
          return { layout };
        },
        resetHomeLayout: () => {
          layout = defaultLayout();
          return {};
        },
      },
    ),
  );
  return saves;
}

function shownSwitch(home: HTMLElement, name: string) {
  return within(home).getByRole("switch", { name: `Show ${name}` });
}

test("a User hides a Home row and Featured on Settings, then resets to the default", async () => {
  const saves = openSettings();
  const home = await screen.findByRole("region", { name: "Home" });

  fireEvent.click(shownSwitch(home, "Top picks"));
  await expect.poll(() => saves.at(-1)?.rows[1]?.hidden).toBe(true);
  await expect
    .poll(() => shownSwitch(home, "Top picks").getAttribute("aria-checked"))
    .toBe("false");
  fireEvent.click(shownSwitch(home, "Featured"));
  await expect.poll(() => saves.at(-1)?.featuredHidden).toBe(true);
  expect(saves.at(-1)?.rows[1]?.hidden).toBe(true);
  expect(shownSwitch(home, "Continue Watching").getAttribute("aria-checked")).toBe("true");

  fireEvent.click(within(home).getByRole("button", { name: "Reset to default" }));

  await expect.poll(() => shownSwitch(home, "Top picks").getAttribute("aria-checked")).toBe("true");
  expect(shownSwitch(home, "Featured").getAttribute("aria-checked")).toBe("true");
});
