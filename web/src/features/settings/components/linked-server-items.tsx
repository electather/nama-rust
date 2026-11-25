import { LinkedServerItem } from "@/features/settings/components/linked-server-item";
import { useLinkedServers } from "@/features/settings/data/use-linked-servers";

/** The Servers the signed-in Account owns or has a User on, or that it has none yet. */
export function LinkedServerItems() {
  const servers = useLinkedServers();
  if (servers.length === 0) {
    return <p className="py-2.5 text-sm text-muted-foreground">No Server is Linked yet.</p>;
  }
  return (
    <ul aria-label="Your Servers" className="-mb-1.5 flex flex-col">
      {servers.map((server) => (
        <LinkedServerItem key={server.id} server={server} />
      ))}
    </ul>
  );
}
