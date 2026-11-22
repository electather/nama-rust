import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

// The Hub `vp dev` forwards RPCs to, so the browser sees one origin (ADR 0015).
const hubUrl = process.env["NAMA_HUB_URL"] ?? "http://localhost:7981";

// Building only: format, lint, type-check and tests run from the workspace's `vite.config.ts`
// (web/README.md, "Checks"). The Hub embeds `dist/` and serves it (ADR 0010).
export default defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [
    tailwindcss(),
    tanstackRouter({ target: "react", autoCodeSplitting: true }),
    viteReact(),
  ],
  server: { proxy: { "^/nama\\.hub\\.": hubUrl } },
});
