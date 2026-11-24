import { SignOutNotice } from "@hub/features/account/components/sign-out-notice";
import { useSignOutOtherSessions } from "@hub/features/account/data/sessions";
import { Button } from "nama-ui/components/button";

/** Signs out every session of the Account but this browser's. */
export function SignOutEverywhereElse() {
  const signOutOthers = useSignOutOtherSessions();
  return (
    <>
      <Button
        variant="outline"
        disabled={signOutOthers.isPending}
        onClick={() => {
          signOutOthers.mutate({});
        }}
      >
        Sign out everywhere else
      </Button>
      <SignOutNotice error={signOutOthers.error} />
    </>
  );
}
