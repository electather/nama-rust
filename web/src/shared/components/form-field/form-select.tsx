import type { ComponentProps } from "react";

import { Field, type FieldText } from "@/shared/components/form-field/field";

/**
 * A labelled native dropdown in a native form, with its hint and the Server's error. Styled as
 * `ui/input.tsx` is (DESIGN.md, "Inputs").
 */
export function FormSelect({
  label,
  hint,
  error,
  ...select
}: ComponentProps<"select"> & FieldText) {
  return (
    <Field label={label} hint={hint} error={error}>
      {(control) => (
        <select
          {...control}
          {...select}
          className="h-10 w-full min-w-0 rounded-lg border border-input bg-transparent px-3 text-base transition-colors outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25 disabled:cursor-not-allowed disabled:bg-muted disabled:text-muted-foreground aria-invalid:border-destructive"
        />
      )}
    </Field>
  );
}
