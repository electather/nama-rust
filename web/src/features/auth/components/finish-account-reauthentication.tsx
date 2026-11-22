import { useNavigate } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";
import { useEffect, useRef } from "react";

import { CodeLayout } from "@/features/auth/components/code-layout";
import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { useReauthenticate } from "@/shared/hooks/use-reauthenticate";
import { accountSignInErrors, takeAccountSignIn } from "@/shared/lib/account-sign-in";

/**
 * Where the Hub sends this browser back to when “Confirm it’s you” went there: gives this Device
 * Recent Authentication with the Account's approval, and goes back to `redirect` to make the change.
 */
export function FinishAccountReauthentication({
  redirect = "/",
}: {
  redirect: string | undefined;
}) {
  const navigate = useNavigate();
  const { mutate, error } = useReauthenticate(() => {
    void navigate({ href: redirect, replace: true });
  });
  // The secret works once, and StrictMode runs an effect twice.
  const finishing = useRef(false);
  useEffect(() => {
    if (!finishing.current) {
      finishing.current = true;
      mutate({ proof: { case: "accountSignInHandle", value: takeAccountSignIn() } });
    }
  }, [mutate]);

  return (
    <CodeLayout heading="Confirm it’s you">
      {error ? (
        <>
          <FormNotices errors={accountSignInErrors(error)} />
          <a href={redirect} className={buttonVariants({ variant: "outline", size: "lg" })}>
            Go back
          </a>
        </>
      ) : (
        <output className="text-center text-sm text-muted-foreground">Confirming it’s you…</output>
      )}
    </CodeLayout>
  );
}
