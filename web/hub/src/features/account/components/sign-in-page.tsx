import { CredentialsForm } from "@hub/features/account/components/credentials-form";
import { useSignIn } from "@hub/features/account/data/sessions";
import { Link } from "@tanstack/react-router";

/** Signing in to an Account with its email and password. */
export function SignInPage() {
  const signIn = useSignIn();
  return (
    <CredentialsForm
      heading="Sign in"
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      attempt={signIn}
      onSubmit={(credentials) => {
        signIn.mutate(credentials);
      }}
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
