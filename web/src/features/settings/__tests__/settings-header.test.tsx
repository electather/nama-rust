import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { SettingsHeader } from "@/features/settings/components/settings-header";

// What the opened crumb does when it renders: nothing is read, so the test picks success or failure.
const crumb = vi.hoisted(() => ({ rendered: 0, fails: false }));

vi.mock(import("@/features/settings/components/plugin-crumbs"), () => ({
  PluginCrumbs: () => {
    crumb.rendered += 1;
    if (crumb.fails) {
      throw new Error("refused");
    }
    return <span>Opened Plugin</span>;
  },
}));

vi.mock(import("@/features/settings/components/user-crumbs"), () => ({
  UserCrumbs: () => <span>Opened User</span>,
}));

afterEach(() => {
  cleanup();
  crumb.rendered = 0;
  crumb.fails = false;
  vi.restoreAllMocks();
});

const noop = vi.fn<() => void>();

function header(served: boolean) {
  render(<SettingsHeader titleId="t" section="plugins" served={served} onClose={noop} />);
}

test("an opened crumb shows when its read succeeds", () => {
  header(true);

  expect(screen.getByText("Opened Plugin")).toBeDefined();
});

test("an unserved Section shows its plain title and never renders the opened crumb", () => {
  header(false);

  expect(screen.getByText("Plugins")).toBeDefined();
  expect(crumb.rendered).toBe(0);
});

test("an opened crumb whose read throws falls back to the plain title", () => {
  // React logs the error it hands to the boundary.
  vi.spyOn(console, "error").mockReturnValue();
  crumb.fails = true;
  header(true);

  expect(screen.getByText("Plugins")).toBeDefined();
  expect(screen.queryByText("Opened Plugin")).toBeNull();
});

test("a failed crumb doesn't carry over to the next Section opened", () => {
  vi.spyOn(console, "error").mockReturnValue();
  crumb.fails = true;
  const { rerender } = render(
    <SettingsHeader titleId="t" section="plugins" served onClose={noop} />,
  );
  rerender(<SettingsHeader titleId="t" section="users" served onClose={noop} />);

  expect(screen.getByText("Opened User")).toBeDefined();
});
