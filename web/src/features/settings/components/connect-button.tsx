import { Button } from "nama-ui/components/button";

import type { useConnectionChanges } from "@/features/settings/hooks/use-connection-changes";

/** Starts connecting the User to the Plugin `name`, which opens its link. */
export function ConnectButton({
  pluginId,
  name,
  start,
}: {
  pluginId: string;
  name: string;
  start: ReturnType<typeof useConnectionChanges>["start"];
}) {
  return (
    <Button
      variant="secondary"
      disabled={start.isPending}
      onClick={() => {
        start.mutate({ pluginId });
      }}
    >
      {start.isPending ? `Opening ${name}…` : `Connect in ${name}`}
    </Button>
  );
}
