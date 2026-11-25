import { ServerItem } from "@/features/servers/components/server-item";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";

/** The Account's Servers, each loading on its own, so one that can't be reached holds up none. */
export function ServerList() {
  const servers = useLinkedServers();
  return (
    servers.length > 0 && (
      <ul aria-label="Your Servers" className="flex flex-col gap-5">
        {servers.map((server) => (
          <ServerItem key={server.id} server={server} />
        ))}
      </ul>
    )
  );
}
