import { Link } from "@tanstack/react-router";

import { EmailField } from "@/features/auth/components/email-field";
import { useCreateAccount } from "@/features/auth/data/use-create-account";
import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";
import { hubErrors } from "@/features/auth/lib/hub-errors";
import { PasswordField } from "@/shared/components/form-field/password-field";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { formText } from "@/shared/lib/forms";
import { textLink } from "@/shared/lib/text-link-classes";

/**
 * Creating an Account at the Hub with an email and a password, which signs it in and goes on to
 * `redirect`.
 */
export function CreateAccount({ redirect }: { redirect: string | undefined }) {
  const create = useCreateAccount(useFinishSignIn(redirect));
  const errors = create.error ? hubErrors(create.error) : {};
  return (
    <SignInLayout
      heading="Create an Account"
      action={create.isPending ? "Creating your Account…" : "Create Account"}
      pending={create.isPending}
      errors={errors}
      onSubmit={(form) => {
        create.mutate({ email: formText(form, "email"), password: formText(form, "password") });
      }}
      footer={
        <p className="text-center text-sm text-muted-foreground">
          Have an Account?{" "}
          <Link to="/sign-in" search={{ redirect }} className={textLink}>
            Sign in
          </Link>
        </p>
      }
    >
      <EmailField error={errors.email} />
      <PasswordField
        label="Password"
        variant="lg"
        autoComplete="new-password"
        hint="8 to 1024 characters."
        error={errors.password}
      />
    </SignInLayout>
  );
}
