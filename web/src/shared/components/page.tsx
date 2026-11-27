import type { ReactNode } from "react";

import { Backdrop } from "@/shared/components/backdrop";
import { PageTitle } from "@/shared/components/page-title";

/**
 * A page's main column, under its heading and the `art` beside it, if any; the heading also names
 * the browser tab. With a `backdrop` URL, the heading sits on it, at its bottom start.
 */
export function Page({
  heading,
  art,
  backdrop,
  children,
}: {
  heading: string;
  art?: ReactNode;
  backdrop?: { url: string | undefined; thumbhash: Uint8Array | undefined };
  children: ReactNode;
}) {
  const banner = backdrop?.url !== undefined;
  return (
    <main className="mx-auto w-full max-w-5xl px-4 py-10 sm:px-6">
      <PageTitle name={heading} />
      <div
        className="relative isolate flex items-center gap-6 data-banner:h-48 data-banner:items-end data-banner:overflow-hidden data-banner:rounded-3xl data-banner:p-5 sm:data-banner:h-64"
        data-banner={banner ? "" : undefined}
      >
        {/* Only a page with a Backdrop asks the Server for its image widths: an error page may
            show because the Server can't be reached. */}
        {backdrop !== undefined && (
          <Backdrop
            url={backdrop.url}
            thumbhash={backdrop.thumbhash}
            // The column is at most 1024px wide, less its 24px sides.
            sizes="(min-width: 1024px) 976px, 100vw"
          />
        )}
        {art}
        <h1 className="text-2xl font-semibold tracking-tight text-balance">{heading}</h1>
      </div>
      {children}
    </main>
  );
}
