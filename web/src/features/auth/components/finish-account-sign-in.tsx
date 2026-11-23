import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";
import { useEffect, useRef } from "react";

import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";
import { useSignInWithAccount } from "@/features/auth/data/use-sign-in-with-account";
import { SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";
import { accountSignInErrors, takeAccountSignIn } from "@/shared/lib/account-sign-in";

/**
 * Where the Hub sends this browser back to: finishing the sign-in with an Account it started, with
 * the Hub's one-time `code`.
 */
export function FinishAccountSignIn({
  redirect,
  code,
}: {
  redirect: string | undefined;
  code: string;
}) {
  const { mutate, error } = useSignInWithAccount(useFinishSignIn(redirect));
  // The secret works once, and StrictMode runs an effect twice.
  const finishing = useRef(false);
  useEffect(() => {
    if (!finishing.current) {
      finishing.current = true;
      mutate({ handle: takeAccountSignIn(), code, delivery: SessionDelivery.COOKIE });
    }
  }, [mutate, code]);

  return (
    <SignInCard heading="Sign in with your Account">
      {error ? (
        <>
          <FormNotices errors={accountSignInErrors(error)} />
          <Link
            to="/sign-in"
            search={{ redirect }}
            className={buttonVariants({ variant: "outline", size: "lg" })}
          >
            Back to sign in
          </Link>
        </>
      ) : (
        <output className="text-center text-sm text-muted-foreground">Signing you in…</output>
      )}
    </SignInCard>
  );
}
