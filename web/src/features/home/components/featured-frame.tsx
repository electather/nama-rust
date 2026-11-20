import type { ReactNode } from "react";

/** The hero's row of panels, across the page's column: side by side, or stacked on a phone. */
export function FeaturedFrame({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section aria-label={label} className="mx-auto w-full max-w-350 px-5 pt-3.5">
      <ul className="@container flex h-145 gap-2.5 perspective-hero max-[720px]:h-auto max-[720px]:flex-col max-[720px]:perspective-none">
        {children}
      </ul>
    </section>
  );
}
