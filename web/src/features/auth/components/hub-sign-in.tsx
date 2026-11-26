import { EmailField } from "@/features/auth/components/email-field";
import { OtherWaysIn } from "@/features/auth/components/other-ways-in";
import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";
import { useSignInAtHub } from "@/features/auth/data/use-sign-in-at-hub";
import { hubErrors } from "@/features/auth/lib/hub-errors";
import { FormField } from "@/shared/components/form-field/form-field";
import { SignInLayout } from "@/shared/components/sign-in-layout/sign-in-layout";
import { formText } from "@/shared/lib/forms";

/**
 * The sign-in page at the Hub: an Account's email and password, or else a Sign-in Link, on a Hub
 * that `sendsSignInLinks`, or a passkey, on one that `offersPasskeys`.
 */
export function HubSignIn({
  redirect,
  sendsSignInLinks,
  offersPasskeys,
}: {
  redirect: string | undefined;
  sendsSignInLinks: boolean;
  offersPasskeys: boolean;
}) {
  const signIn = useSignInAtHub(useFinishSignIn(redirect));
  const errors = signIn.error ? hubErrors(signIn.error) : {};
  return (
    <SignInLayout
      heading="Sign in to the Hub"
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      pending={signIn.isPending}
      errors={errors}
      onSubmit={(form) => {
        signIn.mutate({ email: formText(form, "email"), password: formText(form, "password") });
      }}
      footer={
        <OtherWaysIn
          redirect={redirect}
          sendsSignInLinks={sendsSignInLinks}
          offersPasskeys={offersPasskeys}
        />
      }
    >
      <EmailField error={errors.email} />
      <FormField
        label="Password"
        name="password"
        type="password"
        variant="lg"
        autoComplete="current-password"
        error={errors.password}
      />
    </SignInLayout>
  );
}
