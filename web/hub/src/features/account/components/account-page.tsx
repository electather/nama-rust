import { DeleteAccount } from "@hub/features/account/components/delete-account";
import { PasswordForm } from "@hub/features/account/components/password-form";
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
 * The signed-in Account's email, the Users created from it waiting for it, its password, Linking a
 * Server and the Servers it Linked, Linking a User and the Users it Linked, whether the Movies they
 * finish are pooled, the way to its viewing history, signing out, and deleting it.
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
