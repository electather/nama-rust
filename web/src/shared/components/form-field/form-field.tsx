import type { ComponentProps } from "react";

import { Field, type FieldText } from "@/shared/components/form-field/field";
import { Input } from "@/shared/ui/input";

/**
 * A labelled text input in a native form, with its hint and the Server's error. It is required
 * unless `required` says otherwise.
 */
export function FormField({
  label,
  hint,
  error,
  aside,
  heading,
  ...input
}: ComponentProps<typeof Input> & FieldText) {
  return (
    <Field label={label} hint={hint} error={error} aside={aside} heading={heading ?? false}>
      {(control) => <Input {...control} {...input} />}
    </Field>
  );
}
