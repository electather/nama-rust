import { ConnectionRow } from "@/features/settings/components/connection-row";
import { usePluginConnections } from "@/features/settings/data/use-plugin-connections";

/**
 * The signed-in User's Plugin Connections, such as their Telegram chat, each to connect or
 * disconnect. Nothing while no enabled Plugin connects Users.
 */
export function PluginConnections() {
  const { data } = usePluginConnections();
  if (!data || data.connections.length === 0) {
    return null;
  }
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
