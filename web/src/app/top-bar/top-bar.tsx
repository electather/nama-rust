import type { ReactNode } from "react";

import { useLifted } from "@/app/top-bar/use-lifted";

/**
 * The bar across the top of every signed-in page, holding its `children` at either end. Flush and
 * transparent at rest, it lifts into a tray once the page scrolls.
 */
export function TopBar({ children }: { children: ReactNode }) {
  return (
    <header
      data-lifted={useLifted() || undefined}
      className="group sticky top-0 z-20 flex justify-center transition-all duration-300 ease-out-expo data-lifted:px-5 data-lifted:pt-2.5 motion-reduce:transition-none"
    >
      <div className="w-full max-w-350 outline -outline-offset-1 outline-transparent transition-all duration-300 ease-out-expo group-data-lifted:rounded-xl group-data-lifted:bg-secondary group-data-lifted:p-1 group-data-lifted:shadow-float group-data-lifted:outline-border motion-reduce:transition-none">
        <div className="flex items-center justify-between gap-5 px-5 py-3.5 transition-all duration-300 ease-out-expo group-data-lifted:rounded-lg group-data-lifted:bg-card group-data-lifted:px-3.5 group-data-lifted:py-2 motion-reduce:transition-none">
          {children}
        </div>
      </div>
    </header>
  );
}
