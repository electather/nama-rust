import { LeaveButton } from "@/features/settings/components/leave-button";
import { OpenLink } from "@/features/settings/components/open-link";
import { useConnectionChanges } from "@/features/settings/hooks/use-connection-changes";
import { pluginName, status } from "@/features/settings/lib/connection-wording";
import { type PluginConnection, PluginConnectionState } from "@/gen/nama/v1/user_pb";
import { Button } from "@/shared/ui/button";

/** One Plugin's connection: where it is, and the buttons to connect, cancel or disconnect. */
export function ConnectionRow({
  connection: { pluginId, state },
}: {
  connection: PluginConnection;
}) {
  const name = pluginName(pluginId);
  const { start, remove, failed } = useConnectionChanges(name);
  const waiting = state === PluginConnectionState.WAITING;

  return (
    <li className="flex min-h-14 flex-wrap items-center gap-x-3 gap-y-1 border-b py-2 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="font-medium">{name}</p>
        <p className="text-sm text-muted-foreground">{status(state, name)}</p>
        <p role="alert" className="text-sm text-destructive empty:hidden">
          {failed}
        </p>
      </div>
      {/* Known only to the page that made it: after a reload, Cancel and Connect make another. */}
      {waiting && start.data && <OpenLink href={start.data.url} name={name} />}
      {state === PluginConnectionState.NOT_CONNECTED ? (
        <Button
          variant="ghost"
          disabled={start.isPending}
          onClick={() => {
            start.mutate({ pluginId });
          }}
        >
          Connect {name}
        </Button>
      ) : (
        <LeaveButton
          waiting={waiting}
          name={name}
          disabled={remove.isPending}
          onClick={() => {
            remove.mutate({ pluginId });
          }}
        />
      )}
    </li>
  );
}
