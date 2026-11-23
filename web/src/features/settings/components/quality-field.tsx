import type { ReactNode } from "react";

/** What a Fulfiller's Quality is for, the Plugin it's from under it, and its dropdown at the end. */
export function QualityField({
  id,
  name: { label, from },
  control,
}: {
  id: string;
  name: { label: string; from: string };
  control: ReactNode;
}) {
  return (
    <div className="flex items-center gap-3">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <label id={`${id}-label`} htmlFor={id} className="truncate text-sm">
          {label}
        </label>
        <span id={`${id}-hint`} className="truncate text-xs text-muted-foreground">
          {from}
        </span>
      </div>
      <div className="flex w-39 shrink-0">{control}</div>
    </div>
  );
}
