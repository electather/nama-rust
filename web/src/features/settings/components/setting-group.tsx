import type { ReactNode } from "react";

/** Rows of a pane that belong together, under a `title`. */
export function SettingGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col">
      <h4 className="font-medium">{title}</h4>
      <div className="flex flex-col">{children}</div>
    </div>
  );
}
