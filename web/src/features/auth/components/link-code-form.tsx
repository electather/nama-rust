import { useId } from "react";

import { LinkRemedy } from "@/features/auth/components/link-remedy";
import { linkErrors } from "@/features/auth/lib/link-errors";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** Entering a code a Server shows, for itself or for one of its Users, which Links it. */
export function LinkCodeForm({
  heading,
  hint,
  action,
  linked,
  pending,
  error,
  onCode,
}: {
  heading: string;
  /** Where the Server shows the code. */
  hint: string;
  /** What the submit button says, such as “Link Server”. */
  action: string;
  /** What to say once the code Linked something. */
  linked: string | undefined;
  pending: boolean;
  /** The Hub's refusal of the last code, if it refused it. */
  error: unknown;
  /** Sends the code typed in `form`. */
  onCode: (code: string, form: HTMLFormElement) => void;
}) {
  const id = useId();
  const errors = error === null ? {} : linkErrors(error);
  return (
    <form
      aria-labelledby={id}
      className="flex flex-col gap-3"
      onSubmit={onSubmitted((typed, form) => {
        onCode(formText(typed, "linkCode"), form);
      })}
    >
      <h2 id={id} className="text-lg font-semibold">
        {heading}
      </h2>
      <FormField
        label="Code"
        name="linkCode"
        variant="code"
        autoComplete="off"
        autoCapitalize="characters"
        spellCheck={false}
        hint={hint}
        error={errors.linkCode}
      />
      <FormEnd
        variant="ghost"
        action={pending ? "Linking…" : action}
        pending={pending}
        done={linked}
        refusal={errors.form}
      >
        <LinkRemedy error={error} />
      </FormEnd>
    </form>
  );
}
