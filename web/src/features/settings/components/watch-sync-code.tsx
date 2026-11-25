import { Button, ghostHang } from "nama-ui/components/button";

import type { StartWatchSyncLinkResponse } from "@/gen/nama/v1/user_pb";

/** The code the User enters at their tracker to link their account, while it waits, and cancelling. */
export function WatchSyncCode({
  started,
  tracker,
  onCancel,
}: {
  started: StartWatchSyncLinkResponse;
  /** The Plugin's name, such as Trakt. */
  tracker: string;
  onCancel: () => void;
}) {
  const minutes = Math.max(Math.round(started.expiresInSeconds / 60), 1);
  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm text-pretty">
        Open{" "}
        <a
          href={started.verificationUrl}
          target="_blank"
          rel="noreferrer"
          className="font-medium underline underline-offset-4"
        >
          {tracker}
        </a>
        , sign in and enter this code:
      </p>
      <p className="text-2xl font-medium tracking-wide tabular-nums">{started.userCode}</p>
      <p className="text-xs text-muted-foreground">
        Waiting for it to be entered. It works for{" "}
        {minutes === 1 ? "a minute" : `${minutes} minutes`}.
      </p>
      <div className={ghostHang}>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
