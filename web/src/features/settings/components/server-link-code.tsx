import { Button } from "nama-ui/components/button";

import type { ShownLink } from "@/features/settings/lib/server-link";

/** The code the Server shows while it waits for someone to enter it on the Hub, and cancelling. */
export function ServerLinkCode({
  link,
  enterUnder,
  pending,
  onCancel,
}: {
  link: ShownLink;
  /** The Hub's form the code goes in, such as “Link a Server”. */
  enterUnder: string;
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
        , sign in to your Account and enter this code under “{enterUnder}”:
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
