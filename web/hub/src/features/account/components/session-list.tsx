import { SessionRow } from "@hub/features/account/components/session-row";
import { SignOutEverywhereElse } from "@hub/features/account/components/sign-out-everywhere-else";
import { SignOutNotice } from "@hub/features/account/components/sign-out-notice";
import { useSessions, useSignOutSession } from "@hub/features/account/data/sessions";

/** Where the Account is signed in, each other session with a button that signs it out. */
export function SessionList() {
  const listed = useSessions();
  const signOut = useSignOutSession();
  if (listed.isPending) {
    return <p className="text-sm text-muted-foreground">Looking up where you’re signed in…</p>;
  }
  if (listed.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        The Hub couldn’t list where you’re signed in just now. Reload the page to try again.
      </p>
    );
  }
  const { sessions } = listed.data;
  return (
    <>
      <ul className="flex flex-col divide-y">
        {sessions.map((session) => (
          <SessionRow
            key={session.id}
            session={session}
            signingOut={signOut.isPending}
            onSignOut={() => {
              signOut.mutate({ sessionId: session.id });
            }}
          />
        ))}
      </ul>
      <SignOutNotice error={signOut.error} />
      {sessions.some((session) => !session.isCurrent) && <SignOutEverywhereElse />}
    </>
  );
}
