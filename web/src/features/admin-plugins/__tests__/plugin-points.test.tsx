import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ExtensionPoints } from "@/features/admin-plugins/components/extension-points";
import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import { openPlugins, row } from "@/test-plugins";

afterEach(cleanup);

test("an Automation Plugin shows the Automation point on the list", async () => {
  openPlugins();

  const found1 = await row("cleanup");
  expect(found1.getByText("Automation")).toBeDefined();
});

test("a Recogniser shows the Recogniser point", () => {
  render(<ExtensionPoints extensionPoints={[ExtensionPoint.RECOGNISER]} />);

  expect(screen.getByText("Recogniser")).toBeDefined();
});

test("a subtitle source shows the Subtitle source point", () => {
  render(<ExtensionPoints extensionPoints={[ExtensionPoint.SUBTITLE_SOURCE]} />);

  expect(screen.getByText("Subtitle source")).toBeDefined();
});
