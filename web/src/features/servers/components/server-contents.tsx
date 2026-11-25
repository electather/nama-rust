import { LibraryLinks } from "@/features/servers/components/library-links";
import type { OpensAloneWhy } from "@/features/servers/data/server-reach";
import { useServerReach } from "@/features/servers/data/use-server-reach";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { ExternalTextLink } from "@/shared/components/external-text-link";

const ownWebAppLink = {
  outdated: "Update this Server",
  "plain-http": "Open this Server",
} satisfies Record<OpensAloneWhy, string>;

/** What the Account can browse on `server`, or why it can't from the Hub. */
export function ServerContents({ server }: { server: LinkedServer }) {
  const reached = useServerReach(server);
  if (reached.kind === "opens-alone") {
    // Its first address is the one the page tried, so its own web app is there.
    return (
      <ExternalTextLink href={server.addresses[0] ?? ""} className="text-sm">
        {ownWebAppLink[reached.why]}
      </ExternalTextLink>
    );
  }
  if (reached.kind === "no-user") {
    return (
      <p className="text-sm text-muted-foreground">Your Account has no User on this Server.</p>
    );
  }
  return <LibraryLinks serverId={server.id} transport={reached.transport} />;
}
