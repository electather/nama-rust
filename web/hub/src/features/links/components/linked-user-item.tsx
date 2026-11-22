import { useUnlinkLinkedUser } from "@hub/features/links/data/users";
import type { LinkedUser } from "@hub/gen/nama/hub/v1/link_pb";
import { Button } from "nama-ui/components/button";

const linkedOn = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

/** One Linked User: who, on which Server, since when, and Unlinking them. */
export function LinkedUserItem({ user }: { user: LinkedUser }) {
  const unlink = useUnlinkLinkedUser();
  return (
    <li className="flex items-center justify-between gap-3 py-2.5">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-base wrap-break-word">
          {user.name} on {user.serverName}
        </span>
        <span className="text-xs text-muted-foreground">
          Linked {linkedOn.format(new Date(Number(user.linkedAt) * 1000))}
        </span>
        <p role="alert" className="text-sm text-destructive empty:hidden">
          {unlink.error ? "The Hub couldn’t Unlink this User just now. Try again." : undefined}
        </p>
      </div>
      <Button
        variant="ghost"
        aria-label={`Unlink ${user.name} on ${user.serverName}`}
        disabled={unlink.isPending}
        onClick={() => {
          unlink.mutate({ id: user.id });
        }}
      >
        {unlink.isPending ? "Unlinking…" : "Unlink"}
      </Button>
    </li>
  );
}
