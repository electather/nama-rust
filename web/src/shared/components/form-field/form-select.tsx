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
          className="h-8 w-full min-w-0 rounded-lg border border-input bg-input/30 px-2.5 text-base text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive/50 aria-invalid:ring-3 aria-invalid:ring-destructive/40 md:text-sm"
        />
      )}
    </Field>
  );
}
