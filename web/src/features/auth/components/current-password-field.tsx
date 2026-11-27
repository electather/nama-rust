import { Button } from "nama-ui/components/button";
import { useId, useState } from "react";

import { FormField } from "@/shared/components/form-field/form-field";

/**
 * The sign-in password field. "Forgot your password?" opens `remedy`, what to do instead, and is
 * left out where there's none.
 */
export function CurrentPasswordField({
  error,
  remedy,
}: {
  error: string | undefined;
  remedy: string | undefined;
}) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <FormField
      label="Password"
      name="password"
      type="password"
      variant="lg"
      autoComplete="current-password"
      error={error}
      hint={open && <span id={id}>{remedy}</span>}
      aside={
        remedy !== undefined && (
          <Button
            variant="link"
            size="link"
            aria-expanded={open}
            aria-controls={id}
            onClick={() => {
              setOpen(!open);
            }}
          >
            Forgot your password?
          </Button>
        )
      }
    />
  );
}
