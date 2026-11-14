import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { openPlugins, row } from "./test-plugins";
import { refused } from "./test-refusal";

afterEach(cleanup);

test.each([
  ["ending in a full stop shows one", "the key was turned down."],
  ["without a full stop gets one", "the key was turned down"],
])("a refused-to-start reason %s", async (_, reason) => {
  openPlugins({
    enablePlugin: () => {
      throw refused(Code.FailedPrecondition, "test-plugin refused to start", {
        reason: RefusalReason.PLUGIN_REFUSED_TO_START,
        field: "id",
        params: { pluginId: "test-plugin", reason },
      });
    },
  });
  const testPlugin = await row("test-plugin");
  fireEvent.click(testPlugin.getByRole("button", { name: "Enable test-plugin" }));

  const alert = await testPlugin.findByRole("alert");
  expect(alert.textContent).toBe("It refused to start: the key was turned down.");
});
