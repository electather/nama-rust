import type { ReactNode } from "react";

/** Rows of a pane that belong together, under a `title`, with what acts on all of them at its end. */
export function SettingGroup({
  title,
  action,
  children,
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h4 className="text-sm font-medium whitespace-nowrap text-muted-foreground">{title}</h4>
        {action}
      </div>
      <div className="flex flex-col">{children}</div>
    </div>
  );
}
