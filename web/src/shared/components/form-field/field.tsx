import { type ReactNode, useId } from "react";

import { AlertText } from "@/shared/components/form-field/alert-text";

/** What a field shows around its control: its label, hint and the Server's error. */
export interface FieldText {
  label: string;
  name: string;
  /** Says what the field is for, under it, until an error takes its place. */
  hint?: ReactNode;
  /** The Server's reason for refusing what was typed, if it did. */
  error?: string | undefined;
  /** A quiet button on the label's row, such as "Show" for a password. */
  aside?: ReactNode;
  /** Whether the label heads a step of the form, as an `<h2>`, such as "1. Setup Code". */
  heading?: boolean;
}

/** What a field hands its control, so its label, hint and error name and describe it. */
export interface FieldControl {
  id: string;
  required: true;
  "aria-invalid": boolean;
  "aria-describedby": string;
}

/** A label, hint and error around the control `children` renders, wired to them by id. */
export function Field({
  label,
  hint,
  error,
  aside,
  heading = false,
  children,
}: Omit<FieldText, "name"> & {
  children: (control: FieldControl) => ReactNode;
}) {
  const id = useId();
  return (
    // `aside` follows the control, so Tab reaches the field first, but `order` lifts it onto the
    // label's row.
    <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-2">
      {heading ? (
        <h2 className="order-1 flex-1 text-base font-medium">
          <label htmlFor={id}>{label}</label>
        </h2>
      ) : (
        <label htmlFor={id} className="order-1 flex-1 text-sm font-medium">
          {label}
        </label>
      )}
      <div className="order-3 basis-full">
        {children({
          id,
          required: true,
          "aria-invalid": error !== undefined,
          "aria-describedby": `${id}-hint ${id}-error`,
        })}
      </div>
      {aside !== undefined && <div className="order-2">{aside}</div>}
      {error === undefined && (
        <p
          id={`${id}-hint`}
          className="order-4 basis-full text-xs text-muted-foreground empty:hidden"
        >
          {hint}
        </p>
      )}
      <AlertText id={`${id}-error`} error={error} />
    </div>
  );
}
