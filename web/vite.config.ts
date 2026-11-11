import { readFileSync } from "node:fs";

import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { type Plugin, defineConfig } from "vite-plus";

// The Server `vp dev` forwards RPCs and media to, so the browser sees one origin (ADR 0015).
const serverUrl = process.env["NAMA_SERVER_URL"] ?? "http://localhost:7980";

// The ASS renderer bundles libass, FreeType, HarfBuzz, FriBidi and more, whose licenses ask for
// their notices to ship with it. Its package lists them all in one file.
const libassNotices: Plugin = {
  name: "libass-notices",
  applyToEnvironment: (environment) => environment.name === "client",
  generateBundle() {
    const notices = import.meta.resolve("@jellyfin/libass-wasm/dist/js/COPYRIGHT");
    this.emitFile({
      type: "asset",
      fileName: "licenses/libass-wasm.txt",
      source: readFileSync(new URL(notices)),
    });
  },
};

// CI sets `CI`. There a failed test is retried twice and the results go to a report that
// `nama-test-report` reads (ADR 0022); locally a flaky test fails, so you notice it.
const ci = Boolean(process.env["CI"]);

// Written by `buf generate` and the router plugin, never by hand.
const generated = ["src/gen/**", "src/routeTree.gen.ts"];

export default defineConfig({
  resolve: { tsconfigPaths: true },
  // SPA mode: the build prerenders one `_shell.html` that the Server embeds and serves for every
  // page (ADR 0004). The router's automatic code splitting is on by default.
  plugins: [tailwindcss(), tanstackStart({ spa: { enabled: true } }), viteReact(), libassNotices],
  server: {
    proxy: {
      // Connect paths (`/nama.v1.ServerService/GetServerInfo` and the like) and media: the same
      // paths the Server keeps out of the web app (`crates/nama-server/src/web.rs`).
      "^/nama\\.": serverUrl,
      "/media/": serverUrl,
    },
  },
  // The build prerenders `_shell.html` through a preview server. Where `localhost` resolves to
  // `::1` first, it listens on IPv6 while the prerender's fetch dials 127.0.0.1 and is refused.
  preview: { host: "127.0.0.1" },
  test: {
    environment: "happy-dom",
    retry: ci ? 2 : 0,
    // JSON, not JUnit: vitest's JUnit report doesn't show which tests passed only on a retry.
    reporters: ci ? ["default", "json"] : ["default"],
    outputFile: { json: "test-results/vitest.json" },
  },
  fmt: {
    ignorePatterns: generated,
  },
  lint: {
    ignorePatterns: generated,
    plugins: ["typescript", "react", "jsx-a11y", "import", "unicorn", "promise", "oxc", "vitest"],
    jsPlugins: [
      { name: "vite-plus", specifier: "vite-plus/oxlint-plugin" },
      // Design-system rules, read from components.json and src/styles.css.
      { name: "shadcn", specifier: "@shadcn/lint" },
    ],
    categories: {
      correctness: "error",
      suspicious: "error",
      pedantic: "error",
      perf: "error",
    },
    rules: {
      "vite-plus/prefer-vite-plus-imports": "error",
      "shadcn/no-restyle": "error",
      "shadcn/no-raw-colors": "error",
      "shadcn/no-arbitrary-values": "error",
      "shadcn/no-inline-styles": "error",
      "shadcn/no-unknown-classes": "error",
      "shadcn/require-static-classes": "error",
      // Picked from `restriction`.
      eqeqeq: "error",
      "no-console": "error",
      "no-debugger": "error",
      "no-alert": "error",
      "no-var": "error",
      "typescript/no-explicit-any": "error",
      "typescript/no-non-null-assertion": "error",
      // The router's `redirect()` and `notFound()` are thrown, and aren't `Error`s.
      "typescript/only-throw-error": [
        "error",
        {
          allow: [
            {
              from: "package",
              package: "@tanstack/router-core",
              name: ["Redirect", "NotFoundError"],
            },
          ],
        },
      ],
      // `expectUnderField` (src/test-refusal.ts) asserts for the tests that call it.
      "vitest/expect-expect": ["error", { assertFunctionNames: ["expect", "expectUnderField"] }],
      // The automatic JSX runtime needs no `React` in scope.
      "react/react-in-jsx-scope": "off",
      // React props (`children: ReactNode`) and library callback arguments can't be deeply
      // readonly, so this flags every component and callback.
      "typescript/prefer-readonly-parameter-types": "off",
    },
    options: { typeAware: true, typeCheck: true },
  },
});
