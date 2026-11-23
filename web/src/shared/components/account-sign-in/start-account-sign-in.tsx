import { Button } from "nama-ui/components/button";

import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { useStartAccountSignIn } from "@/shared/hooks/use-start-account-sign-in";
import { accountSignInErrors } from "@/shared/lib/account-sign-in";

/**
 * The button that goes to the Hub to sign in with an Account, which sends this browser back to
 * `returnUrl`, and why it couldn't.
 */
export function StartAccountSignIn({ returnUrl }: { returnUrl: string }) {
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
          start.mutate({ returnUrl });
        }}
      >
        {going ? "Going to the Hub…" : "Sign in with your Account"}
      </Button>
      {start.error && <FormNotices errors={accountSignInErrors(start.error)} />}
    </>
  );
}
