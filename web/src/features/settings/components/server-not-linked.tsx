import { Button, ghostHang } from "nama-ui/components/button";

import { ServerLinkRow } from "@/features/settings/components/server-link-row";

/**
 * That the Server or a User isn't Linked, with the button that Links it. Given `onRedirect`, it
 * opens the Hub's approval, and showing a code is the other way.
 */
export function ServerNotLinked({
  pending,
  onStart,
  onRedirect,
}: {
  pending: boolean;
  /** Starts Linking by a code. */
  onStart: () => void;
  onRedirect: (() => void) | undefined;
}) {
  return (
    <>
      <ServerLinkRow
        says={<span className="text-muted-foreground">Not Linked</span>}
        action="Link to the Hub"
        doing={onRedirect ? "Linking…" : "Getting a code…"}
        variant="secondary"
        pending={pending}
        onClick={onRedirect ?? onStart}
      />
      {onRedirect && (
        // For an Admin on another device than the Server's browser, or off its Server URL.
        <div className={ghostHang}>
          <Button variant="ghost" disabled={pending} onClick={onStart}>
            Show a code instead
          </Button>
        </div>
      )}
    </>
  );
}
