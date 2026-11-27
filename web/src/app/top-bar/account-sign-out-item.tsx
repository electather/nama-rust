import { DropdownMenuItem } from "nama-ui/components/dropdown-menu";

import { useSignOutAccount } from "@/features/auth/data/use-sign-out-account";
import { Notice } from "@/shared/components/sign-in-layout/notice";

const failed = (
  <div className="max-w-60 pt-1">
    <Notice kind="error">The Hub couldn’t sign you out just now. Try again.</Notice>
  </div>
);

/** The user menu's Sign out at the Hub, which signs the Account out, or says why it couldn't. */
export function AccountSignOutItem() {
  const signOut = useSignOutAccount();
  return (
    <>
      <DropdownMenuItem
        disabled={signOut.isPending}
        // The menu stays open, so it can say why the Account isn't signed out.
        closeOnClick={false}
        onClick={() => {
          signOut.mutate({});
        }}
      >
        {signOut.isPending ? "Signing out…" : "Sign out"}
      </DropdownMenuItem>
      {signOut.error && failed}
    </>
  );
}
