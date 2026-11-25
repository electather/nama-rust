import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { ExternalTextLink } from "@/shared/components/external-text-link";

/** In place of a Section `server` doesn't serve, being older than the page at the Hub. */
export function UpdateServer({ server }: { server: LinkedServer }) {
  return (
    <div className="flex flex-col items-start gap-2 p-4 text-sm md:px-5 md:pt-2">
      <p className="text-muted-foreground">This Server is too old to change this here.</p>
      {/* Its first address is the one the page reached it at, so its own web app is there. */}
      <ExternalTextLink href={server.addresses[0] ?? ""}>Update this Server</ExternalTextLink>
    </div>
  );
}
