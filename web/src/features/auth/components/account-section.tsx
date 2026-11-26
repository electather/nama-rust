import { Link } from "@tanstack/react-router";
import { Button, buttonVariants } from "nama-ui/components/button";
import { useId } from "react";

import { AccountLinks } from "@/features/auth/components/account-links";
import { LinkServerForm } from "@/features/auth/components/link-server-form";
import { LinkUserForm } from "@/features/auth/components/link-user-form";
import { useSignOutAccount } from "@/features/auth/data/use-sign-out-account";
import { AccountServers } from "@/features/servers/components/account-servers";
import { Notice } from "@/shared/components/sign-in-layout/notice";

/**
 * The Account with `email` signed in, on Home at the Hub: who it is, its Servers' Libraries, its Watchlist across them, the
 * Users waiting for it, Linking a Server or a User by the code it shows, its Settings, and signing
 * it out.
 */
export function AccountSection({ email }: { email: string }) {
  const signOut = useSignOutAccount();
  const headingId = useId();
  return (
    <section
      aria-labelledby={headingId}
      className="flex w-full max-w-110 flex-col gap-5 self-center rounded-xl border bg-card p-7.5"
    >
      <h2 id={headingId} className="text-center text-3xl font-medium tracking-tight text-balance">
        Your Account
      </h2>
      <dl className="flex flex-col gap-1">
        <dt className="text-xs text-muted-foreground">Email</dt>
        <dd className="text-base break-all">{email}</dd>
      </dl>
      <AccountServers />
      <Link to="/watchlist" className={buttonVariants({ variant: "outline", size: "lg" })}>
        Watchlist
      </Link>
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
    </section>
  );
}
