import { cn } from "cn";
import { type ReactNode, useId } from "react";

import type { ControlSize } from "@/shared/lib/control-size";

/** What a row hands its control, so the label and hint name and describe it. */
export interface RowControl {
  id: string;
  "aria-labelledby": string;
  "aria-describedby": string;
}

// A hairline between rows only: none over the first one, under a heading.
const ROW = "flex items-center gap-x-5 gap-y-2.5 py-3.5 not-first:border-t";

// Each `ControlSize`'s width.
const SIZES: Record<ControlSize, string> = {
  field: "w-40 md:w-60",
  number: "w-36 md:w-32",
  own: "",
};

/**
 * One setting, in Settings or in a Plugin's own settings: its `label` and `hint`, and at the end
 * the control `children` renders, as wide as its `size` says, at the row's end. A `wide` control,
 * such as a row of choices, drops under its label on a phone, and a sized one then fills the row.
 */
export function SettingRow({
  label,
  hint,
  error,
  wide = false,
  size = "own",
  children,
}: {
  label: string;
  hint?: ReactNode;
  /** What's wrong with the setting as it is, in red under its hint. */
  error?: string | undefined;
  wide?: boolean;
  size?: ControlSize;
  children: (control: RowControl) => ReactNode;
}) {
  const id = useId();
  return (
    <div className={wide ? `${ROW} max-md:flex-col max-md:items-stretch` : ROW}>
      <div className="min-w-0 flex-1">
        <label id={`${id}-label`} htmlFor={id} className="block text-sm">
          {label}
        </label>
        <span id={`${id}-hint`} className="block text-xs text-pretty text-muted-foreground">
          {hint}
        </span>
        <span
          id={`${id}-error`}
          role="alert"
          className="block text-xs text-pretty text-destructive empty:hidden"
        >
          {error}
        </span>
      </div>
      <div
        className={cn(
          "flex shrink-0 justify-end max-md:justify-start",
          SIZES[size],
          wide && size !== "own" && "max-md:w-full",
        )}
      >
        {children({
          id,
          "aria-labelledby": `${id}-label`,
          "aria-describedby": `${id}-hint ${id}-error`,
        })}
      </div>
    </div>
  );
}
