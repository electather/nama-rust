import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ExtensionPoints } from "@/components/plugin-at-a-glance";
import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";

import { openPlugins, row } from "./test-plugins";

afterEach(cleanup);

test("an Automation Plugin shows the Automation point on the list", async () => {
  openPlugins();

  expect((await row("cleanup")).getByText("Automation")).toBeDefined();
});

test("a Recogniser shows the Recogniser point", () => {
  render(<ExtensionPoints extensionPoints={[ExtensionPoint.RECOGNISER]} />);

  expect(screen.getByText("Recogniser")).toBeDefined();
});
