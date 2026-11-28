import type { ReactNode } from "react";

/**
 * A card's `caption` at its bottom start and `menu`, its "…", at the end of its last line, on a
 * band that blurs and darkens the art behind them toward the scrim, as tall as the caption plus a
 * fade above it, so a long caption reads over any art (DESIGN.md, "The Legible Over Art Rule").
 */
export function CardCaption({ caption, menu }: { caption: ReactNode; menu: ReactNode }) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-0">
      <div aria-hidden className="absolute inset-0 mask-t-from-20% backdrop-blur-sm" />
      <div aria-hidden className="absolute inset-0 caption-scrim" />
      <div className="relative flex items-end gap-2 px-1 pt-14 pb-1">
        <div aria-hidden className="flex min-w-0 flex-1 flex-col gap-0.5 ps-2.5 pb-3">
          {caption}
        </div>
        {/* Dark over the art, whatever the page's theme, so it keeps its contrast. */}
        <div className="dark pointer-events-auto shrink-0 rounded-full">{menu}</div>
      </div>
    </div>
  );
}
