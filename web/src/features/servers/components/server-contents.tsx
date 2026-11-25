import { LibraryLinks } from "@/features/servers/components/library-links";
import { useServerConnection } from "@/features/servers/data/use-server-connection";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { ExternalTextLink } from "@/shared/components/external-text-link";

/** What the Account can browse on `server`, or why it can't from the Hub. */
export function ServerContents({ server }: { server: LinkedServer }) {
  const connection = useServerConnection(server);
  if (connection.kind === "outdated") {
    // It was reached at its first address, so its own web app is there.
    return (
      <ExternalTextLink href={server.addresses[0] ?? ""} className="text-sm">
        Update this Server
      </ExternalTextLink>
    );
  }
  if (connection.kind === "no-user") {
    return (
      <p className="text-sm text-muted-foreground">Your Account has no User on this Server.</p>
    );
  }
  return <LibraryLinks serverId={server.id} transport={connection.transport} />;
}
