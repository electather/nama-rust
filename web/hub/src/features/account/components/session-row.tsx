import type { Session } from "@hub/gen/nama/hub/v1/account_pb";
import { Button } from "nama-ui/components/button";

const lastUsed = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/** One place the Account is signed in, by its browser, and, unless it's this one, signing it out. */
export function SessionRow({
  session,
  signingOut,
  onSignOut,
}: {
  session: Session;
  /** Whether a session is being signed out, this one or another. */
  signingOut: boolean;
  onSignOut: () => void;
}) {
  return (
    <li className="flex items-center justify-between gap-3 py-1">
      <span className="flex flex-col">
        <span className="text-sm font-medium">{session.name}</span>
        <span className="text-xs text-muted-foreground">
          {session.isCurrent
            ? "This browser"
            : `Last used ${lastUsed.format(new Date(Number(session.lastUsedAt) * 1000))}`}
        </span>
      </span>
      {!session.isCurrent && (
        <Button
          variant="ghost"
          aria-label={`Sign out ${session.name}`}
          disabled={signingOut}
          onClick={onSignOut}
        >
          Sign out
        </Button>
      )}
    </li>
  );
}
