import { CircleDashed, Clock } from "lucide-react";
import type { ReactNode } from "react";

/** Under the code: that it's waiting to be approved and `expiresIn`, or that it expired. */
export function CodeStatus({ expired, expiresIn }: { expired: boolean; expiresIn: ReactNode }) {
  return expired ? (
    <output className="flex items-center gap-2 text-sm">
      <Clock aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      This code has expired.
    </output>
  ) : (
    <div className="flex flex-wrap items-center justify-center gap-x-3.5 gap-y-1 text-sm">
      <output className="flex items-center gap-2">
        <CircleDashed aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        Waiting for it to be approved…
      </output>
      {expiresIn}
    </div>
  );
}
