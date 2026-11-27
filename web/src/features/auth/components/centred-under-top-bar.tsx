import type { ReactNode } from "react";

/** A page's one card, `children`, centred on the screen under the top bar. */
export function CentredUnderTopBar({ children }: { children: ReactNode }) {
  return (
    // Starts under the top bar, 72px (`h-18`), so it centres on the whole screen.
    <div className="-mt-18 flex min-h-dvh items-center justify-center px-5 pt-30 pb-12">
      {children}
    </div>
  );
}
