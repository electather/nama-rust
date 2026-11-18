import { HeadContent, Scripts, createRootRouteWithContext } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { NotFound } from "@/components/route-error";
import type { RouterContext } from "@/lib/access";

import appCss from "../styles.css?url";

export const Route = createRootRouteWithContext<RouterContext>()({
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { title: "Nama" },
      // Living-room black (DESIGN.md), as in `public/manifest.webmanifest`: a meta tag can't read a token.
      { name: "theme-color", content: "#0a0a0a" },
    ],
    links: [
      { rel: "manifest", href: "/manifest.webmanifest" },
      { rel: "icon", href: "/icon.svg", type: "image/svg+xml" },
      { rel: "apple-touch-icon", href: "/apple-touch-icon.png" },
      // Under `vp test` vitest stubs CSS, so this is `""`. An empty href makes React warn, and a real
      // one hangs every test: the router marks a stylesheet with `precedence`, and React then waits
      // for a load that never comes in happy-dom.
      ...(appCss ? [{ rel: "stylesheet", href: appCss }] : []),
    ],
  }),
  notFoundComponent: NotFound,
  shellComponent: RootDocument,
});

function RootDocument({ children }: { children: ReactNode }) {
  return (
    // Dark only for now (ADR 0016).
    <html lang="en" className="dark">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  );
}
