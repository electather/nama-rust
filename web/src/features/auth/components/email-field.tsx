import type { ReactNode } from "react";

import { FormField } from "@/shared/components/form-field/form-field";

/** An Account's email, the name it signs in with at the Hub. */
export function EmailField({ error, hint }: { error: string | undefined; hint?: ReactNode }) {
  return (
    <FormField
      label="Email"
      name="email"
      type="email"
      variant="lg"
      autoComplete="username"
      spellCheck={false}
      hint={hint}
      error={error}
    />
  );
}
