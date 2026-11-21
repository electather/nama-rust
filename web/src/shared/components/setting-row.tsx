import { type ReactNode, useId } from "react";

/** What a row hands its control, so the label and hint name and describe it. */
export interface RowControl {
  id: string;
  "aria-labelledby": string;
  "aria-describedby": string;
}

// A hairline between rows only: none over the first one, under a heading.
const ROW = "flex items-center gap-x-5 gap-y-2.5 py-3.5 not-first:border-t";

/**
 * One setting, in Settings or in a Plugin's own settings: its `label` and `hint`, and at the end
 * the control `children` renders. A `wide` control, such as a row of choices, drops under its label
 * on a phone.
 */
export function SettingRow({
  label,
  hint,
  error,
  wide = false,
  children,
}: {
  label: string;
  hint?: ReactNode;
  /** What's wrong with the setting as it is, in red under its hint. */
  error?: string | undefined;
  wide?: boolean;
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
      <div className="flex shrink-0 justify-end max-md:justify-start">
        {children({
          id,
          "aria-labelledby": `${id}-label`,
          "aria-describedby": `${id}-hint ${id}-error`,
        })}
      </div>
    </div>
  );
}
