import { ServerLibraries } from "@/features/servers/components/server-libraries";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";

/** One of the Account's Servers under its name. */
export function ServerItem({ server }: { server: LinkedServer }) {
  return (
    <li className="flex flex-col items-start gap-2.5">
      <h2 className="text-lg font-semibold">{server.name}</h2>
      <ServerLibraries server={server} />
    </li>
  );
}
