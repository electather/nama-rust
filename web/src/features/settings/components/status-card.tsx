import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { IconTile } from "@/shared/components/icon-tile";

/**
 * How something a pane is about stands, on a card at its top: an icon, a `title` over the lines
 * `children` says, and what to do about it at the end, which wraps under them when narrow.
 * `?setting=` finds it by its `setting` key.
 */
export function StatusCard({
  setting,
  Icon,
  destructive = false,
  title,
  end,
  children,
}: {
  setting?: string;
  Icon: LucideIcon;
  /** It's something that failed, in Destructive. */
  destructive?: boolean;
  title: string;
  end: ReactNode;
  children: ReactNode;
}) {
  return (
    <div
      data-setting={setting}
      className="flex flex-wrap items-center gap-3 rounded-xl border bg-card px-3.5 py-3"
    >
      <IconTile Icon={Icon} destructive={destructive} />
      <div className="flex min-w-0 flex-1 basis-50 flex-col gap-0.5">
        <span className={destructive ? "text-sm text-destructive" : "text-sm"}>{title}</span>
        {children}
      </div>
      {end}
    </div>
  );
}
