import { useAccount } from "@hub/features/account/data/account";
import { useSignOut } from "@hub/features/account/data/sessions";
import { LinkServerForm } from "@hub/features/links/components/link-server-form";
import { LinkUserForm } from "@hub/features/links/components/link-user-form";
import { ServerList } from "@hub/features/links/components/server-list";
import { UserList } from "@hub/features/links/components/user-list";
import { Button } from "nama-ui/components/button";

/**
 * The signed-in Account's email, Linking a Server and the Servers it Linked, Linking a User and the
 * Users it Linked, and signing out.
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
      <LinkServerForm />
      <ServerList />
      <LinkUserForm />
      <UserList />
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
    </>
  );
}
