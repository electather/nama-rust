import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ExternalTextLink } from "@/shared/components/external-text-link";

afterEach(cleanup);

test("an external link opens in a new tab that can't reach the app", () => {
  render(<ExternalTextLink href="https://example.com/">Example</ExternalTextLink>);
  const link = screen.getByRole("link", { name: "Example" });
  expect([
    link.getAttribute("href"),
    link.getAttribute("target"),
    link.getAttribute("rel"),
  ]).toStrictEqual(["https://example.com/", "_blank", "noopener noreferrer"]);
});
