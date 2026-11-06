import tailwindcss from "@tailwindcss/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

// The Server `vp dev` forwards RPCs and media to, so the browser sees one origin (ADR 0015).
const server = process.env["NAMA_SERVER_URL"] ?? "http://localhost:7980";

export default defineConfig({
  resolve: { tsconfigPaths: true },
  // SPA mode: the build prerenders one `_shell.html` that the Server embeds and serves for every
  // page (ADR 0004). The router's automatic code splitting is on by default.
  plugins: [tailwindcss(), tanstackStart({ spa: { enabled: true } }), viteReact()],
  server: {
    proxy: {
      // Connect paths: `/nama.v1.ServerService/GetServerInfo` and the like.
      "^/nama\\.": server,
      "/media/": server,
    },
  },
  // The build prerenders `_shell.html` through a preview server. Where `localhost` resolves to
  // `::1` first, it listens on IPv6 while the prerender's fetch dials 127.0.0.1 and is refused.
  preview: { host: "127.0.0.1" },
  test: {
    environment: "happy-dom",
  },
  fmt: {
    ignorePatterns: ["src/gen/**", "src/routeTree.gen.ts"],
  },
  lint: {
    ignorePatterns: ["src/gen/**", "src/routeTree.gen.ts"],
    plugins: ["typescript", "react", "jsx-a11y", "import", "unicorn", "promise", "oxc", "vitest"],
    jsPlugins: [{ name: "vite-plus", specifier: "vite-plus/oxlint-plugin" }],
    categories: {
      correctness: "error",
      suspicious: "error",
      pedantic: "error",
      perf: "error",
    },
    rules: {
      "vite-plus/prefer-vite-plus-imports": "error",
      // Picked from `restriction`.
      eqeqeq: "error",
      "no-console": "error",
      "no-debugger": "error",
      "no-alert": "error",
      "no-var": "error",
      "typescript/no-explicit-any": "error",
      "typescript/no-non-null-assertion": "error",
      // The automatic JSX runtime needs no `React` in scope.
      "react/react-in-jsx-scope": "off",
      // React props (`children: ReactNode`) and library callback arguments can't be deeply
      // readonly, so this flags every component and callback.
      "typescript/prefer-readonly-parameter-types": "off",
    },
    options: { typeAware: true, typeCheck: true },
  },
});
