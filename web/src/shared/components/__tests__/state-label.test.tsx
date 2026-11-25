import { cleanup, render, screen } from "@testing-library/react";
import { Clock, CircleX } from "lucide-react";
import { afterEach, expect, test } from "vite-plus/test";

import { StateLabel } from "@/shared/components/state-label";

afterEach(cleanup);

test.each([false, true])(
  "a quiet pill is Secondary-filled in Ink, unedged (large: %s)",
  (large) => {
    render(<StateLabel label="Pending approval" Icon={Clock} large={large} />);
    const pill = screen.getByText("Pending approval");
    const has = (name: string) => pill.classList.contains(name);
    expect({
      "bg-secondary": has("bg-secondary"),
      "text-foreground": has("text-foreground"),
      border: has("border"),
    }).toStrictEqual({ "bg-secondary": true, "text-foreground": true, border: false });
  },
);

test("a destructive pill keeps the 15% red tint off Popover", () => {
  render(<StateLabel tone="destructive" label="Failed" Icon={CircleX} />);
  expect(screen.getByText("Failed").classList.contains("bg-destructive/15")).toBe(true);
});
