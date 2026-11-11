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
    ],
    // Under `vp test` vitest stubs CSS, so this is `""`. An empty href makes React warn, and a real
    // one hangs every test: the router marks a stylesheet with `precedence`, and React then waits for
    // a load that never comes in happy-dom.
    links: appCss ? [{ rel: "stylesheet", href: appCss }] : [],
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
