import type { ReactNode } from "react";

/** One fact of Playback info: its `name`, then the fact. Nothing when it isn't known. */
export function InfoRow({ name, children }: { name: string; children: ReactNode }) {
  if (children === undefined) {
    return null;
  }
  return (
    <div className="flex gap-3.5">
      <dt className="w-18 shrink-0 text-muted-foreground">{name}</dt>
      <dd>{children}</dd>
    </div>
  );
}
