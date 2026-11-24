import { Input } from "nama-ui/components/input";
import type { ComponentProps, ReactNode } from "react";

import { Field, type FieldControl, type FieldText } from "@/shared/components/form-field/field";
import { FieldNumber } from "@/shared/components/form-field/field-number";
import { RowField } from "@/shared/components/form-field/row-field";
import { inputLayout } from "@/shared/lib/control-size";

/**
 * A labelled text input in a native form, with its hint and the Server's error. It is required
 * unless `required` says otherwise. As a `row` it's drawn as a setting's row, for a form in Settings,
 * found by its `setting` key, holding its `hiddenInput`. A `type="number"` one has − and + at its
 * end (`ui/number-field.tsx`), which `onStep` hears.
 */
export function FormField({
  label,
  hint,
  error,
  aside,
  heading,
  row = false,
  setting,
  hiddenInput,
  onStep,
  ...input
}: ComponentProps<typeof Input> &
  FieldText & {
    row?: boolean;
    setting?: string;
    hiddenInput?: ReactNode;
    onStep?: (value: string, field: HTMLInputElement | null) => void;
  }) {
  const frame = { label, hint, error, aside, heading: heading ?? false };
  const number = input.type === "number";
  const control = (given: FieldControl) =>
    number ? (
      <FieldNumber control={given} input={input} onStep={onStep} />
    ) : (
      <Input {...given} {...input} />
    );
  return row ? (
    <RowField setting={setting} hiddenInput={hiddenInput} {...frame} {...inputLayout(input.type)}>
      {control}
    </RowField>
  ) : (
    <Field {...frame}>{control}</Field>
  );
}
