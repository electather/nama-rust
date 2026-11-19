import type { ConnectError } from "@connectrpc/connect";

import { CodeField } from "@/features/auth/components/code-field";
import { codeErrors } from "@/features/auth/lib/sign-in-errors";
import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { type FormErrors, formText, onSubmitted } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

/** The form that takes the Sign-in Code another Device shows. */
export function LookUp({
  code,
  onSubmit,
  pending,
  error,
  refused,
}: {
  /** What the field starts with: a scanned code, or one the Server refused to sign in. */
  code: string | undefined;
  onSubmit: (code: string) => void;
  pending: boolean;
  error: ConnectError | null;
  /** Why the Server refused to approve `code`, until it's looked up again. */
  refused?: FormErrors | undefined;
}) {
  const errors = error === null ? (refused ?? {}) : codeErrors(error);
  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        onSubmit(formText(typed, "signInCode"));
      })}
    >
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">
        Sign in a Device
      </h1>
      <CodeField
        label="Sign-in Code"
        name="signInCode"
        length={8}
        placeholder="XXXX-XXXX"
        defaultValue={code}
        hint="It’s on the screen of the Device you’re signing in."
        error={errors.signInCode}
      />
      <FormNotices errors={errors} />
      <Button type="submit" size="lg" disabled={pending}>
        {pending ? "Looking it up…" : "Continue"}
      </Button>
    </form>
  );
}
