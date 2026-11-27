import { LinkServerPrompt } from "@/features/servers/components/link-server-prompt";
import { ServerNotice } from "@/features/servers/components/server-notice";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";

/**
 * A notice for each of the Account's Servers Home can't show, each as it's reached, so one that
 * can't be holds up none; or, with no Server, Linking one.
 */
export function LinkedServerNotices() {
  const servers = useLinkedServers();
  if (servers.length === 0) {
    return <LinkServerPrompt />;
  }
  return (
    // Hidden while every Server shows, so Home keeps no gap for it.
    <div className="flex flex-col gap-2 empty:hidden">
      {servers.map((server) => (
        <ServerNotice key={server.id} server={server} />
      ))}
    </div>
  );
}
