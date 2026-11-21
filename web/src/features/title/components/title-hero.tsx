import type { ReactNode } from "react";

import { TitleBackdrop } from "@/features/title/components/title-backdrop";

/**
 * A title page's top: its Backdrop full width, from under the app bar, on the scrim, with
 * `children`, its heading, facts and actions, at its bottom start. Without a Backdrop, `children`
 * alone on the page.
 */
export function TitleHero({
  backdrop,
  children,
}: {
  backdrop: { url: string | undefined; thumbhash: Uint8Array | undefined };
  children: ReactNode;
}) {
  const art = backdrop.url !== undefined;
  return (
    <div
      className="relative isolate -mt-18 flex flex-col justify-end overflow-hidden pt-28 data-art:min-h-184"
      data-art={art ? "" : undefined}
    >
      <TitleBackdrop url={backdrop.url} thumbhash={backdrop.thumbhash} />
      {art && <div aria-hidden className="absolute inset-0 -z-10 title-scrim" />}
      <div className="mx-auto w-full max-w-350 px-5 pb-10">
        <div className="flex max-w-140 flex-col gap-3.5">{children}</div>
      </div>
    </div>
  );
}
