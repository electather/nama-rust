import { Textarea } from "nama-ui/components/textarea";
import type { ComponentProps } from "react";

import { Field, type FieldText } from "@/shared/components/form-field/field";

/** A labelled textarea in a native form, with its hint, the Server's error and its `aside`. */
export function FormTextarea({
  label,
  hint,
  error,
  aside,
  ...textarea
}: ComponentProps<"textarea"> & FieldText) {
  return (
    <Field label={label} hint={hint} error={error} aside={aside}>
      {(control) => <Textarea {...control} {...textarea} />}
    </Field>
  );
}
