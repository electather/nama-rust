import { LinkControls } from "@/features/settings/components/link-controls";
import { ServerNames } from "@/features/settings/components/server-names";
import { ServerReachability } from "@/features/settings/components/server-reachability";
import { useServerLink } from "@/features/settings/data/use-server-link";
import { useStartServerLink } from "@/features/settings/data/use-start-server-link";
import { useUnlinkServer } from "@/features/settings/data/use-unlink-server";
import { linkFailure } from "@/features/settings/lib/server-link";

/**
 * Whether the Server is Linked to an Account on the Hub, and Linking or Unlinking it. Shown only
 * while a Plugin that links the Server, the Hub Plugin, is enabled.
 */
export function ServerLink() {
  const link = useServerLink();
  const start = useStartServerLink();
  const unlink = useUnlinkServer();
  if (link === undefined) {
    return null;
  }
  return (
    <section aria-labelledby="server-link" className="flex flex-col gap-3.5">
      <div>
        <h4 id="server-link" className="text-xs text-muted-foreground">
          Hub
        </h4>
        <p className="mt-1 text-sm text-pretty text-muted-foreground">
          What the Hub adds needs the Server Linked to your Account there. Without it, the Server
          works just the same.
        </p>
      </div>
      <LinkControls
        link={link}
        enterUnder="Link a Server"
        start={start}
        unlink={unlink}
        failure={linkFailure(start.error ?? unlink.error)}
      />
      <ServerReachability link={link} />
      <ServerNames link={link} />
    </section>
  );
}
