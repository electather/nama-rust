import { LinkedServerItem } from "@hub/features/links/components/linked-server-item";
import { useLinkedServers } from "@hub/features/links/data/servers";

/** The signed-in Account's Linked Servers, each with when it was Linked. */
export function ServerList() {
  return (
    <section aria-labelledby="your-servers" className="flex flex-col gap-2">
      <h2 id="your-servers" className="text-lg/6 font-medium">
        Your Servers
      </h2>
      <Servers />
    </section>
  );
}

/** The list itself, or what stands in for it while it loads or if it can't. */
function Servers() {
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
