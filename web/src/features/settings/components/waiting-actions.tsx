import { Clock } from "lucide-react";
import { Button } from "nama-ui/components/button";

import type { useConnectionChanges } from "@/features/settings/hooks/use-connection-changes";
import { ExternalTextLink } from "@/shared/components/external-text-link";
import { StateLabel } from "@/shared/components/state-label";

/** While a connection's link waits for the User: that it does, Cancel, and the link again. */
export function WaitingActions({
  pluginId,
  name,
  changes: { start, remove },
}: {
  pluginId: string;
  name: string;
  changes: ReturnType<typeof useConnectionChanges>;
}) {
  return (
    <>
      <StateLabel tone="warning" label="Waiting for you" Icon={Clock} />
      <Button
        variant="ghost"
        aria-label={`Cancel connecting ${name}`}
        disabled={remove.isPending}
        onClick={() => {
          remove.mutate({ pluginId });
        }}
      >
        {remove.isPending ? "Cancelling…" : "Cancel"}
      </Button>
      {/* Known only to the page that made it: after a reload, Cancel and Connect make another. */}
      {start.data && <ExternalTextLink href={start.data.url}>Open {name} again</ExternalTextLink>}
    </>
  );
}
