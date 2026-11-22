import type { ReactNode } from "react";

import type { FieldControl, FieldText } from "@/shared/components/form-field/field";
import { SettingRow } from "@/shared/components/setting-row";
import type { ControlSize } from "@/shared/lib/control-size";

/**
 * `Field` as a setting's row, for a form in Settings: the label and hint at the start, and at the
 * end the control `children` renders, as wide as `size` says: a field's, unless it's a number's. A
 * row has no aside and is never a heading.
 */
export function RowField({
  label,
  hint,
  error,
  size = "field",
  children,
}: Omit<FieldText, "name"> & {
  size?: ControlSize;
  children: (control: FieldControl) => ReactNode;
}) {
  return (
    <SettingRow label={label} hint={hint} error={error} size={size}>
      {(control) => (
        <>
          {children({
            id: control.id,
            required: true,
            "aria-invalid": error !== undefined,
            "aria-describedby": control["aria-describedby"],
          })}
        </>
      )}
    </SettingRow>
  );
}
