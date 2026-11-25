import { LibraryLinks } from "@/features/servers/components/library-links";
import { useServerReach } from "@/features/servers/data/use-server-reach";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { ExternalTextLink } from "@/shared/components/external-text-link";

/** What the Account can browse on `server`, or why it can't from the Hub. */
export function ServerContents({ server }: { server: LinkedServer }) {
  const reached = useServerReach(server);
  if (reached.kind === "outdated") {
    // It was reached at its first address, so its own web app is there.
    return (
      <ExternalTextLink href={server.addresses[0] ?? ""} className="text-sm">
        Update this Server
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
