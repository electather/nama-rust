import { CredentialsForm } from "@hub/features/account/components/credentials-form";
import { OrEmailASignInLink } from "@hub/features/account/components/or-email-a-sign-in-link";
import { useSignIn } from "@hub/features/account/data/sessions";
import { useSendsSignInLinks } from "@hub/features/account/data/sign-in-links";
import { Link } from "@tanstack/react-router";

/** Signing in to an Account with its email and password, then going on to the Hub path `continueTo`. */
export function SignInPage({ continueTo }: { continueTo: string }) {
  const signIn = useSignIn(continueTo);
  const sendsSignInLinks = useSendsSignInLinks();
  return (
    <CredentialsForm
      heading="Sign in"
      action={signIn.isPending ? "Signing in…" : "Sign in"}
      attempt={signIn}
      onSubmit={(credentials) => {
        signIn.mutate(credentials);
      }}
      // Unknown hides it: password sign-in works on every Hub, a Sign-in Link only on one with mail.
      alternative={
        sendsSignInLinks === true ? <OrEmailASignInLink continueTo={continueTo} /> : undefined
      }
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
