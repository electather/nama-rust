import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { SaveToast } from "@/features/settings/components/save-toast";

afterEach(cleanup);

test("the toast's Not saved label sits on a 10% red tint, which holds 4.5:1 on Popover", () => {
  render(<SaveToast says="The Server refused it." onDismiss={() => {}} />);
  const pill = screen.getByText("Not saved");
  expect({
    "bg-destructive/10": pill.classList.contains("bg-destructive/10"),
    "bg-destructive/15": pill.classList.contains("bg-destructive/15"),
  }).toStrictEqual({ "bg-destructive/10": true, "bg-destructive/15": false });
});
