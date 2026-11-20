import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginAbout } from "@/features/admin-plugins/components/plugin-about";
import { openPlugin, plugin } from "@/test-plugins";

afterEach(cleanup);

const noAbout = { setupMarkdown: "", homepageUrl: "", sourceUrl: "", license: "" };

/** TMDB's page, its Plugin carrying `about`. */
async function openTmdbWith(about: Partial<typeof noAbout>) {
  openPlugin("tmdb", {
    getPlugin: () => ({ plugin: { summary: { id: "tmdb", name: "TMDB" }, ...about } }),
  });
  const page = await plugin("TMDB");
  return page;
}

/** Renders the About section of a Plugin with only `setupMarkdown`. */
function renderSetup(setupMarkdown: string) {
  return render(<PluginAbout plugin={{ ...noAbout, setupMarkdown }} />);
}

test("a Plugin's setup steps, links and licence show above its settings", async () => {
  const page = await openTmdbWith({
    setupMarkdown: "Get an API key:\n\n1. Sign in to **TMDB**.\n2. Copy the `API key`.",
    homepageUrl: "https://www.themoviedb.org/",
    sourceUrl: "https://github.com/electather/nama-rust",
    license: "MIT",
  });

  const about = page.getByRole("region", { name: "About" });
  expect(about.textContent).toContain("Get an API key:");
  expect(
    within(about)
      .getAllByRole("listitem")
      .map((item) => item.textContent),
  ).toStrictEqual(["Sign in to TMDB.", "Copy the API key."]);
  expect(page.getByText("TMDB", { selector: "strong" })).toBeDefined();
  expect(page.getByText("API key", { selector: "code" })).toBeDefined();
  expect(page.getByRole("link", { name: "Homepage" }).getAttribute("href")).toBe(
    "https://www.themoviedb.org/",
  );
  expect(page.getByRole("link", { name: "Source" }).getAttribute("rel")).toBe(
    "noopener noreferrer",
  );
  expect(page.getByText("MIT licence")).toBeDefined();
  const settings = page.getByRole("region", { name: "Settings" });
  // oxlint-disable-next-line no-bitwise -- `compareDocumentPosition` answers with a bit mask.
  const following = about.compareDocumentPosition(settings) & Node.DOCUMENT_POSITION_FOLLOWING;
  expect(following).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
});

test("a Plugin without setup steps or links shows no About section", async () => {
  const page = await openTmdbWith({});

  expect(page.queryByRole("region", { name: "About" })).toBeNull();
});

test("a Plugin with only a licence shows just the licence", async () => {
  const page = await openTmdbWith({ license: "Apache-2.0" });

  expect(page.getByText("Apache-2.0 licence")).toBeDefined();
  expect(page.queryByRole("link", { name: "Homepage" })).toBeNull();
});

test("raw HTML in setup steps reaches the page as text, never as an element", () => {
  const { container } = renderSetup(
    [
      "<script>alert(1)</script>",
      '<img src="x" onerror="alert(1)">',
      '<iframe src="https://example.com"></iframe>',
      "[click](javascript:alert(1)) and [data](data:text/html,hi)",
    ].join("\n\n"),
  );

  expect(container.querySelector("script, img, iframe")).toBeNull();
  expect(screen.getByText("<script>alert(1)</script>")).toBeDefined();
  expect(screen.queryAllByRole("link")).toStrictEqual([]);
  expect(container.querySelector("[href]")).toBeNull();
  expect(screen.getByText("click and data")).toBeDefined();
});

test("setup steps link only to http and https URLs, opening apart", () => {
  renderSetup("See [the docs](https://example.com/docs) or [the FTP](ftp://example.com).");

  const docs = screen.getByRole("link", { name: "the docs" });
  expect([
    docs.getAttribute("href"),
    docs.getAttribute("rel"),
    docs.getAttribute("target"),
  ]).toStrictEqual(["https://example.com/docs", "noopener noreferrer", "_blank"]);
  expect(screen.queryByRole("link", { name: "the FTP" })).toBeNull();
});

test("a homepage that isn't http or https isn't linked", () => {
  const { container } = render(
    // oxlint-disable-next-line no-script-url -- the URL is the hostile input under test
    <PluginAbout plugin={{ ...noAbout, homepageUrl: "javascript:alert(1)" }} />,
  );

  expect(container.innerHTML).toBe("");
});

test("a numbered list with blank lines and CRLFs between its steps stays one list", () => {
  renderSetup("1. Sign in.\r\n\r\n2. Copy the key\r\n   from your profile.");

  expect(screen.getAllByRole("list")).toHaveLength(1);
  expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toStrictEqual([
    "Sign in.",
    "Copy the key from your profile.",
  ]);
});
