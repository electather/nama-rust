import { Check } from "lucide-react";

import { ConnectButton } from "@/features/settings/components/connect-button";
import { Disconnect } from "@/features/settings/components/disconnect";
import { WaitingActions } from "@/features/settings/components/waiting-actions";
import type { useConnectionChanges } from "@/features/settings/hooks/use-connection-changes";
import { type PluginConnection, PluginConnectionState } from "@/gen/nama/v1/user_pb";
import { StateLabel } from "@/shared/components/state-label";

/**
 * A connection's state and what can be done about it: connect, or while its link waits cancel or
 * open it again, or once connected disconnect.
 */
export function ConnectionActions({
  connection: { pluginId, state },
  name,
  changes,
}: {
  connection: PluginConnection;
  name: string;
  changes: ReturnType<typeof useConnectionChanges>;
}) {
  if (state === PluginConnectionState.CONNECTED) {
    return (
      <>
        <StateLabel tone="positive" label="Connected" Icon={Check} />
        <Disconnect pluginId={pluginId} name={name} remove={changes.remove} />
      </>
    );
  }
  return state === PluginConnectionState.WAITING ? (
    <WaitingActions pluginId={pluginId} name={name} changes={changes} />
  ) : (
    <ConnectButton pluginId={pluginId} name={name} start={changes.start} />
  );
}
