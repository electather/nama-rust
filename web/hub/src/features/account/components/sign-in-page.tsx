import { CredentialsForm } from "@hub/features/account/components/credentials-form";
import { OtherWaysToSignIn } from "@hub/features/account/components/other-ways-to-sign-in";
import { useSignIn } from "@hub/features/account/data/sessions";
import { Link } from "@tanstack/react-router";

/**
 * Signing in to an Account with its email and password, or with a passkey, then going on to the Hub
 * path `continueTo`.
 */
export function SignInPage({ continueTo }: { continueTo: string }) {
  const signIn = useSignIn(continueTo);
  return (
    <CredentialsForm
      heading="Sign in"
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      attempt={signIn}
      onSubmit={(credentials) => {
        signIn.mutate(credentials);
      }}
      alternative={<OtherWaysToSignIn continueTo={continueTo} />}
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
