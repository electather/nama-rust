import { Textarea } from "nama-ui/components/textarea";
import type { ComponentProps } from "react";

import { Field, type FieldText } from "@/shared/components/form-field/field";

/** A labelled textarea in a native form, with its hint and the Server's error. */
export function FormTextarea({
  label,
  hint,
  error,
  ...textarea
}: ComponentProps<"textarea"> & FieldText) {
  return (
    <Field label={label} hint={hint} error={error}>
      {(control) => <Textarea {...control} {...textarea} />}
    </Field>
  );
}
