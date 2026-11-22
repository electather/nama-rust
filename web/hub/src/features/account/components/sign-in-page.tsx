import { CredentialsForm } from "@hub/features/account/components/credentials-form";
import { OrEmailASignInLink } from "@hub/features/account/components/or-email-a-sign-in-link";
import { useSignIn } from "@hub/features/account/data/sessions";
import { useSendsSignInLinks } from "@hub/features/account/data/sign-in-links";
import { Link } from "@tanstack/react-router";

/** Signing in to an Account with its email and password. */
export function SignInPage() {
  const signIn = useSignIn();
  const sendsSignInLinks = useSendsSignInLinks();
  return (
    <CredentialsForm
      heading="Sign in"
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      attempt={signIn}
      onSubmit={(credentials) => {
        signIn.mutate(credentials);
      }}
      alternative={sendsSignInLinks ? <OrEmailASignInLink /> : undefined}
      footer={
        <>
          New to the Hub?{" "}
          <Link to="/create-account" className="text-primary underline underline-offset-3">
            Create an Account
          </Link>
        </>
      }
    />
  );
}
