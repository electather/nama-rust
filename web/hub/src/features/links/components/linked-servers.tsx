import { LinkedServerItem } from "@hub/features/links/components/linked-server-item";
import { useLinkedServers } from "@hub/features/links/data/servers";

/** The Servers the signed-in Account owns or has a User on, or what stands in for them while they load or if they can't. */
export function LinkedServers() {
  const servers = useLinkedServers();
  if (servers.isPending) {
    return <p className="text-sm text-muted-foreground">Looking up your Servers…</p>;
  }
  if (servers.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        The Hub couldn’t list your Servers just now. Reload the page to try again.
      </p>
    );
  }
  return servers.data.servers.length === 0 ? (
    <p className="text-sm text-muted-foreground">No Server is Linked yet.</p>
  ) : (
    <ul className="flex flex-col divide-y">
      {servers.data.servers.map((server) => (
        <LinkedServerItem key={server.id} server={server} />
      ))}
    </ul>
  );
}
