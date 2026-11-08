import { createConnectQueryKey, useMutation, useQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import { type PluginConnection, PluginConnectionState, UserService } from "@/gen/nama/v1/user_pb";
import { refusal } from "@/lib/forms";

const connectionsKey = createConnectQueryKey({
  schema: UserService.method.listPluginConnections,
  cardinality: "finite",
});

/** How often to look again while a link waits for the User to finish in the other app. */
const WAITING_POLL_MS = 2000;

/**
 * The signed-in User's Plugin Connections, such as their Telegram chat, each to connect or
 * disconnect. Nothing while no enabled Plugin connects Users.
 */
export function PluginConnections() {
  const { data } = useQuery(
    UserService.method.listPluginConnections,
    {},
    {
      refetchInterval: (query) =>
        query.state.data?.connections.some(
          (connection) => connection.state === PluginConnectionState.WAITING,
        ) === true
          ? WAITING_POLL_MS
          : false,
    },
  );
  if (!data || data.connections.length === 0) return null;
  return (
    <section aria-labelledby="connections" className="mt-12 max-w-2xl">
      <h2 id="connections" className="font-medium">
        Connections
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        Connect an app, and Nama can message you there about your Requests.
      </p>
      <ul className="mt-4 flex flex-col">
        {data.connections.map((connection) => (
          <ConnectionRow key={connection.pluginId} connection={connection} />
        ))}
      </ul>
    </section>
  );
}

function ConnectionRow({ connection: { pluginId, state } }: { connection: PluginConnection }) {
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

/** The link to finish connecting in the other app, in a new tab. */
function OpenLink({ href, name }: { href: string; name: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="rounded-lg px-4 py-2 text-sm font-medium underline decoration-muted-foreground underline-offset-4 outline-none hover:decoration-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      Open {name}
    </a>
  );
}

/** Cancels the link the User is `waiting` on, or else disconnects them from the Plugin `name`. */
function LeaveButton({
  waiting,
  name,
  disabled,
  onClick,
}: {
  waiting: boolean;
  name: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      variant="ghost"
      aria-label={`${waiting ? "Cancel connecting" : "Disconnect"} ${name}`}
      disabled={disabled}
      onClick={onClick}
    >
      {waiting ? "Cancel" : "Disconnect"}
    </Button>
  );
}

/**
 * Starting and deleting a connection to the Plugin `name`, each listing the connections again once
 * it's done, and why the last one failed, if it did.
 */
function useConnectionChanges(name: string) {
  const queryClient = useQueryClient();
  const onSettled = () => queryClient.invalidateQueries({ queryKey: connectionsKey });
  const start = useMutation(UserService.method.startPluginConnection, {
    // Opened once the link is made, so a browser may block it; the link stays on the page.
    onSuccess: ({ url }) => window.open(url, "_blank", "noopener"),
    onSettled,
  });
  const remove = useMutation(UserService.method.deletePluginConnection, { onSettled });
  const error = start.error ?? remove.error;
  const failed = error ? refusal(error, `connect ${name}`, {}).form : undefined;
  return { start, remove, failed };
}

/** Where the User's connection to the Plugin `name` is, in plain words. */
function status(state: PluginConnectionState, name: string) {
  if (state === PluginConnectionState.WAITING) return `Open ${name} and press Start.`;
  return state === PluginConnectionState.CONNECTED ? "Connected" : "Not connected";
}

// shortcut: a Plugin's id capitalized, which fits Telegram; give Plugins a display name once one
// doesn't read right.
function pluginName(id: string) {
  return `${id.charAt(0).toUpperCase()}${id.slice(1)}`;
}
