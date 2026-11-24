import { Button } from "nama-ui/components/button";

import { HubRefusal } from "@/features/settings/components/hub-refusal";
import { SessionItem } from "@/features/settings/components/session-item";
import { useAccountSessions } from "@/features/settings/data/use-account-sessions";
import { useSignOutOtherSessions } from "@/features/settings/data/use-sign-out-other-sessions";
import { useSignOutSession } from "@/features/settings/data/use-sign-out-session";
import { AlertText } from "@/shared/components/form-field/alert-text";

/** Each browser the Account is signed in on, the others with Sign out, and signing them all out. */
export function SessionItems() {
  const sessions = useAccountSessions();
  const signOut = useSignOutSession();
  const signOutOthers = useSignOutOtherSessions();
  const error = signOut.error ?? signOutOthers.error;
  const signingOut = signOut.isPending || signOutOthers.isPending;
  return (
    <>
      <ul aria-label="Where you’re signed in" className="-mb-1.5 flex flex-col">
        {sessions.map((session) => (
          <SessionItem
            key={session.id}
            session={session}
            signingOut={signingOut}
            onSignOut={() => {
              signOut.mutate({ sessionId: session.id });
            }}
          />
        ))}
      </ul>
      <AlertText
        error={
          <HubRefusal
            error={error}
            said="The Hub couldn’t sign that out just now. Try again."
            toDo="to sign out elsewhere"
          />
        }
      />
      {sessions.some((session) => !session.isCurrent) && (
        <div className="flex justify-end pt-2.5">
          <Button
            variant="outline"
            disabled={signingOut}
            onClick={() => {
              signOutOthers.mutate({});
            }}
          >
            Sign out everywhere else
          </Button>
        </div>
      )}
    </>
  );
}
