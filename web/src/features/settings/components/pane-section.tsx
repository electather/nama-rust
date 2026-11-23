import type { ReactNode } from "react";

/** A part of a pane under its own heading, with what acts on all of it at the heading's end. */
export function PaneSection({
  title,
  action,
  children,
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex min-h-8 items-center justify-between gap-2.5">
        <h4 className="text-base font-medium">{title}</h4>
        {action}
      </div>
      {children}
    </div>
  );
}
