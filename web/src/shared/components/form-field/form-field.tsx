import type { ComponentProps } from "react";

import { Field, type FieldText } from "@/shared/components/form-field/field";
import { RowField } from "@/shared/components/form-field/row-field";
import { Input } from "@/shared/ui/input";

/**
 * A labelled text input in a native form, with its hint and the Server's error. It is required
 * unless `required` says otherwise. As a `row` it's drawn as a setting's row, for a form in Settings.
 */
export function FormField({
  label,
  hint,
  error,
  aside,
  heading,
  row = false,
  ...input
}: ComponentProps<typeof Input> & FieldText & { row?: boolean }) {
  const Frame = row ? RowField : Field;
  return (
    <Frame label={label} hint={hint} error={error} aside={aside} heading={heading ?? false}>
      {(control) => <Input {...control} {...input} />}
    </Frame>
  );
}
