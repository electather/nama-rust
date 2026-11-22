import { Button } from "nama-ui/components/button";

import type { ServerLink } from "@/gen/nama/v1/server_pb";

/** The code the Server shows while it waits for someone to enter it on the Hub, and cancelling. */
export function ServerLinkCode({
  link,
  pending,
  onCancel,
}: {
  link: ServerLink;
  pending: boolean;
  onCancel: () => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm text-pretty">
        Open{" "}
        <a
          href={link.url}
          target="_blank"
          rel="noreferrer"
          className="font-medium underline underline-offset-4"
        >
          the Hub
        </a>
        , sign in to your Account and enter this code:
      </p>
      <p className="text-2xl font-medium tracking-wide tabular-nums">{link.code}</p>
      <p className="text-xs text-muted-foreground">
        Waiting for it to be entered. It works for 10 minutes.
      </p>
      <div className="-ms-4">
        <Button variant="ghost" disabled={pending} onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
