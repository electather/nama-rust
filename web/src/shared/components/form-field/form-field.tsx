import { Input } from "nama-ui/components/input";
import type { ComponentProps } from "react";

import { Field, type FieldControl, type FieldText } from "@/shared/components/form-field/field";
import { RowField } from "@/shared/components/form-field/row-field";

/**
 * A labelled text input in a native form, with its hint and the Server's error. It is required
 * unless `required` says otherwise. As a `row` it's drawn as a setting's row, for a form in Settings,
 * found by its `setting` key.
 */
export function FormField({
  label,
  hint,
  error,
  aside,
  heading,
  row = false,
  setting,
  ...input
}: ComponentProps<typeof Input> & FieldText & { row?: boolean; setting?: string }) {
  const text = { label, hint, error };
  const field = (control: FieldControl) => <Input {...control} {...input} />;
  return row ? (
    <RowField setting={setting} {...text}>
      {field}
    </RowField>
  ) : (
    <Field {...text} aside={aside} heading={heading ?? false}>
      {field}
    </Field>
  );
}
