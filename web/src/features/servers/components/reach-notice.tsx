import type { OpensAloneWhy } from "@/features/servers/data/server-reach";
import { useServerReach } from "@/features/servers/data/use-server-reach";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { ExternalTextLink } from "@/shared/components/external-text-link";
import { Notice } from "@/shared/components/sign-in-layout/notice";

const whyAlone = {
  outdated: "needs an update.",
  "plain-http": "opens only in its own web app.",
} satisfies Record<OpensAloneWhy, string>;

/** Why Home can't show `server`, once the page tried to reach it; nothing if it reached it. */
export function ReachNotice({ server }: { server: LinkedServer }) {
  const reached = useServerReach(server);
  if (reached.kind === "signed-in") {
    return null;
  }
  if (reached.kind === "no-user") {
    return <Notice kind="info">Your Account has no User on {server.name}.</Notice>;
  }
  return (
    <Notice kind="info">
      {server.name} {whyAlone[reached.why]}{" "}
      {/* Its first address is the one the page tried, so its own web app is there. */}
      <ExternalTextLink href={server.addresses[0] ?? ""}>Open {server.name}</ExternalTextLink>
    </Notice>
  );
}
