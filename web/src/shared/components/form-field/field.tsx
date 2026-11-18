import { type ReactNode, useId } from "react";

/** What a field shows around its control: its label, hint and the Server's error. */
export interface FieldText {
  label: string;
  name: string;
  /** Says what the field is for, under it. */
  hint?: string | undefined;
  /** The Server's reason for refusing what was typed, if it did. */
  error?: string | undefined;
}

/** A label, hint and error around the control `children` renders, wired to them by id. */
export function Field({
  label,
  hint,
  error,
  children,
}: Omit<FieldText, "name"> & {
  children: (control: {
    id: string;
    required: true;
    "aria-invalid": boolean;
    "aria-describedby": string;
  }) => ReactNode;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {children({
        id,
        required: true,
        "aria-invalid": error !== undefined,
        "aria-describedby": `${id}-hint ${id}-error`,
      })}
      <p id={`${id}-hint`} className="text-sm text-muted-foreground empty:hidden">
        {hint}
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </div>
  );
}
