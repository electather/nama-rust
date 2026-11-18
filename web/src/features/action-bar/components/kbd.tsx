import { cn } from "cn";
import type { ReactNode } from "react";

/** A key, as the action bar names one. `bordered` gives it a hairline, where it stands alone. */
export function Kbd({ bordered = false, children }: { bordered?: boolean; children: ReactNode }) {
  return (
    <kbd
      className={cn(
        "inline-flex h-5 min-w-5 shrink-0 items-center justify-center gap-1 rounded-xs bg-muted px-1 font-sans text-xs leading-none font-medium text-foreground-soft select-none",
        bordered && "border",
      )}
    >
      {children}
    </kbd>
  );
}
