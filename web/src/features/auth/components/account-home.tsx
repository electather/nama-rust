import { Link } from "@tanstack/react-router";
import { Button, buttonVariants } from "nama-ui/components/button";

import { AccountLinks } from "@/features/auth/components/account-links";
import { LinkServerForm } from "@/features/auth/components/link-server-form";
import { LinkUserForm } from "@/features/auth/components/link-user-form";
import { useSignOutAccount } from "@/features/auth/data/use-sign-out-account";
import { AccountServers } from "@/features/servers/components/account-servers";
import { Notice } from "@/shared/components/sign-in-layout/notice";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";

/**
 * The Hub's page for the Account with `email` signed in: who it is, its Servers' Libraries, the Users waiting for it,
 * Linking a Server or a User by the code it shows, its Settings, and signing it out.
 */
export function AccountHome({ email }: { email: string }) {
  const signOut = useSignOutAccount();
  return (
    <SignInCard heading="Your Account">
      <dl className="flex flex-col gap-1">
        <dt className="text-xs text-muted-foreground">Email</dt>
        <dd className="text-base break-all">{email}</dd>
      </dl>
      <AccountServers />
      <AccountLinks />
      <LinkServerForm />
      <LinkUserForm />
      <Link
        to="."
        search={(rest) => ({ ...rest, settings: "account" })}
        className={buttonVariants({ variant: "outline", size: "lg" })}
      >
        Settings
      </Link>
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
