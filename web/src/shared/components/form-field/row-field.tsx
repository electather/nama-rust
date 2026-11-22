import type { ReactNode } from "react";

import type { FieldControl, FieldText } from "@/shared/components/form-field/field";
import { SettingRow } from "@/shared/components/setting-row";

/**
 * `Field` as a setting's row, for a form in Settings: the label and hint at the start, and at the
 * end the control `children` renders, as wide as a row's dropdown is. A row has no aside and is
 * never a heading.
 */
export function RowField({
  setting,
  label,
  hint,
  error,
  children,
}: Omit<FieldText, "name"> & {
  /** The key `?setting=` finds the row by. */
  setting?: string | undefined;
  children: (control: FieldControl) => ReactNode;
}) {
  return (
    <SettingRow setting={setting} label={label} hint={hint} error={error}>
      {(control) => (
        <span className="flex w-40 md:w-60">
          {children({
            id: control.id,
            required: true,
            "aria-invalid": error !== undefined,
            "aria-describedby": control["aria-describedby"],
          })}
        </span>
      )}
    </SettingRow>
  );
}
