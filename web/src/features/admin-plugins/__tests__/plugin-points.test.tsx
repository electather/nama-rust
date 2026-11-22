import { cleanup } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { openPluginSettings, row } from "@/test-plugins";

afterEach(cleanup);

test("an Automation Plugin shows the Automation point on the list", async () => {
  openPluginSettings();

  const found1 = await row("cleanup");
  expect(found1.getByText("Installed, version 0.1.0 · Automation")).toBeDefined();
});
