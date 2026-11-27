import type { ReactNode } from "react";

import { FormField } from "@/shared/components/form-field/form-field";

/** An Account's email, the name it signs in with at the Hub. */
export function EmailField({
  error,
  hint,
  defaultValue,
}: {
  error: string | undefined;
  hint?: ReactNode;
  defaultValue?: string | undefined;
}) {
  return (
    <FormField
      label="Email"
      name="email"
      type="email"
      variant="lg"
      autoComplete="username"
      spellCheck={false}
      hint={hint}
      defaultValue={defaultValue}
      error={error}
    />
  );
}
