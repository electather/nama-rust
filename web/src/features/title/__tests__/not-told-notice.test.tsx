import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { NotToldNotice } from "@/features/title/components/not-told-notice";

afterEach(cleanup);

test("the not-told notice names what a delete took and who may download it again", () => {
  render(<NotToldNotice deleted="Lies" notTold={["Sonarr"]} />);

  expect(
    screen.getByText("Sonarr couldn’t be told about the delete, so it may download Lies again."),
  ).toBeDefined();
});

test("the not-told notice shows nothing without a delete", () => {
  const { container } = render(<NotToldNotice deleted={undefined} notTold={undefined} />);

  expect(container.textContent).toBe("");
});
