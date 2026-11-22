import { ConnectionItem } from "@/features/settings/components/connection-item";
import { NoConnections } from "@/features/settings/components/no-connections";
import { usePluginConnections } from "@/features/settings/data/use-plugin-connections";

/** The Plugins that can message the signed-in User, each to connect or disconnect, or that none can. */
export function ConnectionList({ managesPlugins }: { managesPlugins: boolean }) {
  const connections = usePluginConnections();
  if (connections.length === 0) {
    return <NoConnections managesPlugins={managesPlugins} />;
  }
  return (
    <ul aria-label="Plugins" className="-my-1.5 flex flex-col">
      {connections.map((connection) => (
        <ConnectionItem key={connection.pluginId} connection={connection} />
      ))}
    </ul>
  );
}
