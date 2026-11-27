import type { ReactNode } from "react";

import { Backdrop } from "@/shared/components/backdrop";
import { PageTitle } from "@/shared/components/page-title";

/**
 * A Library's top: its name, the page's heading, over `count`, both on its Backdrop full width
 * from under the app bar, or on the page when it has none. The name also names the browser tab.
 */
export function LibraryHeader({
  name,
  backdrop,
  count,
}: {
  name: string;
  backdrop: { url: string | undefined; thumbhash: Uint8Array | undefined };
  count: ReactNode;
}) {
  const art = backdrop.url !== undefined;
  return (
    <header
      className="relative isolate flex flex-col justify-end pt-7.5 data-art:-mt-18 data-art:h-65 data-art:overflow-hidden data-art:pt-0 sm:data-art:h-85"
      data-art={art ? "" : undefined}
    >
      <PageTitle name={name} />
      <Backdrop url={backdrop.url} thumbhash={backdrop.thumbhash} sizes="100vw" />
      {/* Darker under the app bar, whose search and buttons sit on the art, and solid page at the
          foot, so the art's last row of pixels doesn't show as a line where the page starts. */}
      {art && (
        <div
          aria-hidden
          className="absolute inset-0 -z-10 bg-linear-to-b from-scrim from-0% via-transparent via-40% to-background to-97%"
        />
      )}
      <div className="mx-auto flex w-full max-w-350 flex-col gap-1 px-5 pb-1">
        <h1 className="text-display font-medium text-balance">{name}</h1>
        <p className="text-sm text-muted-foreground tabular-nums">{count}</p>
      </div>
    </header>
  );
}
