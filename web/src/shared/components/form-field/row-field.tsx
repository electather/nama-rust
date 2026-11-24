import type { ReactNode } from "react";

import type { FieldControl, FieldText } from "@/shared/components/form-field/field";
import { SettingRow } from "@/shared/components/setting-row";
import type { ControlSize } from "@/shared/lib/control-size";

/**
 * `Field` as a setting's row, for a form in Settings: the label and hint at the start, and at the
 * end the control `children` renders, as wide as `size` says: a field's, unless it's a number's. A
 * `wide` control drops under them on a phone and fills the row. A row has no aside and is never a
 * heading.
 */
export function RowField({
  setting,
  label,
  hint,
  error,
  wide = false,
  size = "field",
  hiddenInput,
  children,
}: Omit<FieldText, "name"> & {
  /** The key `?setting=` finds the row by. */
  setting?: string | undefined;
  wide?: boolean;
  size?: ControlSize;
  /**
   * A hidden input the form sends with the setting, kept inside the row: the row's first/last-row
   * styles need the row to be the only element its parent holds.
   */
  hiddenInput?: ReactNode;
  children: (control: FieldControl) => ReactNode;
}) {
  return (
    <SettingRow setting={setting} label={label} hint={hint} error={error} wide={wide} size={size}>
      {(control) => (
        <>
          {children({
            id: control.id,
            required: true,
            "aria-invalid": error !== undefined,
            "aria-describedby": control["aria-describedby"],
          })}
          {hiddenInput}
        </>
      )}
    </SettingRow>
  );
}
