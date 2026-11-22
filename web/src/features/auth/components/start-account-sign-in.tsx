import { Button } from "nama-ui/components/button";

import { useStartAccountSignIn } from "@/features/auth/data/use-start-account-sign-in";
import { accountReturnUrl, accountSignInErrors } from "@/features/auth/lib/account-sign-in";
import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";

/** The button that goes to the Hub to sign in with an Account, and why it couldn't. */
export function StartAccountSignIn({ redirect }: { redirect: string | undefined }) {
  const start = useStartAccountSignIn();
  // Once started, the browser is on its way to the Hub.
  const going = start.isPending || start.isSuccess;
  return (
    <>
      <Button
        variant="outline"
        size="lg"
        disabled={going}
        onClick={() => {
          start.mutate({ returnUrl: accountReturnUrl(redirect) });
        }}
      >
        {going ? "Going to the Hub…" : "Sign in with your Account"}
      </Button>
      {start.error && <FormNotices errors={accountSignInErrors(start.error)} />}
    </>
  );
}
