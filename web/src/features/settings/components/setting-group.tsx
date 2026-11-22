import type { ReactNode } from "react";

/**
 * Rows of a pane that belong together, under a `title`, with what acts on all of them at its end.
 * `?setting=` finds a group that's a setting of its own, such as Invites, by its `setting` key.
 */
export function SettingGroup({
  setting,
  title,
  action,
  children,
}: {
  setting?: string;
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div data-setting={setting} className="flex flex-col">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h4 className="text-sm font-medium whitespace-nowrap text-muted-foreground">{title}</h4>
        {action}
      </div>
      <div className="flex flex-col">{children}</div>
    </div>
  );
}
