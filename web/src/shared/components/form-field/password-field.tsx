import { Button } from "nama-ui/components/button";
import { type ComponentProps, useState } from "react";

import { FormField } from "@/shared/components/form-field/form-field";

/** A password field named `password`, with a Show button so the User can check what they typed. */
export function PasswordField(
  field: Omit<ComponentProps<typeof FormField>, "name" | "type" | "aside">,
) {
  const [shown, setShown] = useState(false);
  return (
    <FormField
      {...field}
      name="password"
      type={shown ? "text" : "password"}
      aside={
        <Button
          variant="link"
          size="link"
          aria-label={shown ? "Hide password" : "Show password"}
          onClick={() => {
            setShown(!shown);
          }}
        >
          {shown ? "Hide" : "Show"}
        </Button>
      }
    />
  );
}
