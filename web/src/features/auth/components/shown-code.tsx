import { cn } from "cn";
import type { ReactNode } from "react";

import { ScanToApprove } from "@/features/auth/components/scan-to-approve";

/** The code, with its QR code until it expires, over `status`, which says where it's got to. */
export function ShownCode({
  code,
  expired,
  children: status,
}: {
  code: string;
  expired: boolean;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex flex-col items-center gap-5 rounded-xl border px-2.5 py-5">
        <p
          className={cn(
            "text-display font-medium tracking-wide whitespace-nowrap tabular-nums",
            expired && "text-muted-foreground line-through",
          )}
        >
          {code}
        </p>
        {!expired && <ScanToApprove code={code} />}
      </div>
      {status}
    </div>
  );
}
