import { AddPasskeyForm } from "@hub/features/account/components/add-passkey-form";
import { DeleteAccount } from "@hub/features/account/components/delete-account";
import { PasskeyList } from "@hub/features/account/components/passkey-list";
import { PasswordForm } from "@hub/features/account/components/password-form";
import { SessionList } from "@hub/features/account/components/session-list";
import { useAccount } from "@hub/features/account/data/account";
import { useSignOut } from "@hub/features/account/data/sessions";
import { LinkServerForm } from "@hub/features/links/components/link-server-form";
import { LinkUserForm } from "@hub/features/links/components/link-user-form";
import { ServerList } from "@hub/features/links/components/server-list";
import { UserInvites } from "@hub/features/links/components/user-invites";
import { UserList } from "@hub/features/links/components/user-list";
import { HistoryLink } from "@hub/features/viewing/components/history-link";
import { PoolingSwitch } from "@hub/features/viewing/components/pooling-switch";
import { Button } from "nama-ui/components/button";

/**
 * The signed-in Account's email, the Users created from it waiting for it, its password, its
 * passkeys, where it's signed in, Linking a Server and the Servers it Linked, Linking a User and
 * the Users it Linked, whether the Movies they finish are pooled, the way to its viewing history,
 * signing out, and deleting it.
 */
export function AccountPage() {
  const account = useAccount();
  const signOut = useSignOut();
  return (
    <>
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">Your Account</h1>
      <dl className="flex flex-col gap-1">
        <dt className="text-xs text-muted-foreground">Email</dt>
        <dd className="text-base break-all">{account?.email}</dd>
      </dl>
      <UserInvites />
      <PasswordForm />
      <section aria-labelledby="your-passkeys" className="flex flex-col gap-2">
        <h2 id="your-passkeys" className="text-lg/6 font-medium">
          Passkeys
        </h2>
        <PasskeyList />
        <AddPasskeyForm />
      </section>
      <section aria-labelledby="your-sessions" className="flex flex-col gap-2">
        <h2 id="your-sessions" className="text-lg/6 font-medium">
          Where you’re signed in
        </h2>
        <SessionList />
      </section>
      <LinkServerForm />
      <ServerList />
      <LinkUserForm />
      <PoolingSwitch />
      <UserList />
      <HistoryLink />
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {signOut.error ? "The Hub couldn’t sign you out just now. Try again." : undefined}
      </p>
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
      <DeleteAccount />
    </>
  );
}
