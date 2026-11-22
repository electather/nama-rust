import { Field, type FieldText } from "@/shared/components/form-field/field";
import { FieldSelect, type SelectChoice } from "@/shared/components/form-field/field-select";
import { RowField } from "@/shared/components/form-field/row-field";

/**
 * A labelled dropdown in a native form, with its hint and the Server's error (DESIGN.md,
 * "Inputs"), drawn by `ui/select.tsx`. As a `row` it's drawn as a setting's row, for a form in
 * Settings. Give it `value` to choose for it; else it starts on `defaultValue`.
 */
export function FormSelect({
  label,
  hint,
  error,
  row = false,
  ...choice
}: FieldText & SelectChoice & { row?: boolean }) {
  const Frame = row ? RowField : Field;
  return (
    <Frame label={label} hint={hint} error={error}>
      {(control) => <FieldSelect control={control} choice={choice} />}
    </Frame>
  );
}
