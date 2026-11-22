import { LinkedServerItem } from "@hub/features/links/components/linked-server-item";
import { useLinkedServers } from "@hub/features/links/data/servers";

/** The signed-in Account's Linked Servers, each with when it was Linked. */
export function ServerList() {
  const servers = useLinkedServers();
  const list =
    servers.length === 0 ? (
      <p className="text-sm text-muted-foreground">No Server is Linked yet.</p>
    ) : (
      <ul className="flex flex-col divide-y">
        {servers.map((server) => (
          <LinkedServerItem key={server.id} server={server} />
        ))}
      </ul>
    );
  return (
    <section aria-labelledby="your-servers" className="flex flex-col gap-2">
      <h2 id="your-servers" className="text-lg/6 font-medium">
        Your Servers
      </h2>
      {list}
    </section>
  );
}
