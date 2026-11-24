import type { ReactNode } from "react";

import { Field, type FieldControl, type FieldText } from "@/shared/components/form-field/field";
import { FieldSelect, type SelectChoice } from "@/shared/components/form-field/field-select";
import { RowField } from "@/shared/components/form-field/row-field";

/**
 * A labelled dropdown in a native form, with its hint and the Server's error (DESIGN.md,
 * "Inputs"), drawn by `ui/select.tsx`. As a `row` it's drawn as a setting's row, for a form in
 * Settings, found by its `setting` key, holding its `hiddenInput`. Give it `value` to choose for
 * it; else it starts on `defaultValue`.
 */
export function FormSelect({
  label,
  hint,
  error,
  row = false,
  setting,
  hiddenInput,
  ...choice
}: FieldText & SelectChoice & { row?: boolean; setting?: string; hiddenInput?: ReactNode }) {
  const text = { label, hint, error };
  const field = (control: FieldControl) => <FieldSelect control={control} choice={choice} />;
  return row ? (
    <RowField setting={setting} hiddenInput={hiddenInput} {...text}>
      {field}
    </RowField>
  ) : (
    <Field {...text}>{field}</Field>
  );
}
