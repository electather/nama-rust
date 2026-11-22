import { Button } from "nama-ui/components/button";
import { useId, useState } from "react";

import { CodeField } from "@/features/auth/components/code-field";

/** Step 1 of setting up: the Setup Code, with "Where’s the log?" saying where to find it. */
export function SetupCodeField({ error }: { error: string | undefined }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <>
      <CodeField
        heading
        label="1. Setup Code"
        name="setupCode"
        length={12}
        placeholder="XXXX-XXXX-XXXX"
        error={error}
        hint="This Server printed it to its log when it started."
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
            Where’s the log?
          </Button>
        }
      />
      {/* Outside the field, so an error, which takes the hint's place, leaves it open. Pulled up
          to the field's own spacing. */}
      {open && (
        <p id={id} className="-mt-3 rounded-xl border px-3.5 py-2.5 text-xs">
          Open the log of the app running Nama: the terminal window, or your NAS or Docker app’s log
          for it. Look for the line that says “create the first Admin with this Setup Code”.
        </p>
      )}
    </>
  );
}
