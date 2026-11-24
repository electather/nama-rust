import { Button } from "nama-ui/components/button";

import { useSignOutAccount } from "@/features/auth/data/use-sign-out-account";
import { Notice } from "@/shared/components/sign-in-layout/notice";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/** The Hub's page for the Account with `email` signed in: who it is, and signing it out. */
export function AccountHome({ email }: { email: string }) {
  const signOut = useSignOutAccount();
  return (
    <SignInCard heading="Your Account">
      <dl className="flex flex-col gap-1">
        <dt className="text-xs text-muted-foreground">Email</dt>
        <dd className="text-base break-all">{email}</dd>
      </dl>
      {signOut.error && (
        <Notice kind="error">The Hub couldn’t sign you out just now. Try again.</Notice>
      )}
      <Button
        variant="outline"
        size="lg"
        disabled={signOut.isPending}
        onClick={() => {
          signOut.mutate({});
        }}
      >
        {signOut.isPending ? "Signing out…" : "Sign out"}
      </Button>
    </SignInCard>
  );
}
