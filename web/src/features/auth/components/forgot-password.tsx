import { useId, useState } from "react";

import { FormField } from "@/shared/components/form-field/form-field";
import { Button } from "@/shared/ui/button";

/** The sign-in password field, with "Forgot your password?" opening who can reset it. */
export function ForgotPassword({ error }: { error: string | undefined }) {
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
      hint={open && <span id={id}>Ask whoever runs this Server to reset it for you.</span>}
      aside={
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
      }
    />
  );
}
