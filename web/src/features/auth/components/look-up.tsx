import type { ConnectError } from "@connectrpc/connect";

import { codeErrors } from "@/features/auth/lib/sign-in-errors";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The form that takes the Sign-in Code another Device shows. */
export function LookUp({
  scanned,
  onSubmit,
  pending,
  error,
}: {
  scanned: string | undefined;
  onSubmit: (code: string) => void;
  pending: boolean;
  error: ConnectError | null;
}) {
  const errors = error === null ? {} : codeErrors(error);
  return (
    <>
      <p className="text-muted-foreground">
        Enter the Sign-in Code the other Device shows to sign it in as you.
      </p>
      <form
        className="mt-6 flex flex-col gap-5"
        onSubmit={onSubmitted((typed) => {
          onSubmit(formText(typed, "signInCode"));
        })}
      >
        <FormField
          label="Sign-in Code"
          name="signInCode"
          placeholder="XXXX-XXXX"
          defaultValue={scanned}
          autoComplete="off"
          autoCapitalize="characters"
          spellCheck={false}
          error={errors.signInCode}
        />
        <FormEnd
          action={pending ? "Looking it up…" : "Continue"}
          pending={pending}
          done={undefined}
          refusal={errors.form}
        />
      </form>
    </>
  );
}
