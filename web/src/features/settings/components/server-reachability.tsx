import type { ServerLink } from "@/gen/nama/v1/server_pb";

/** Whether the Hub reached the Linked Server from outside the LAN; nothing until it tried. */
export function ServerReachability({
  link,
}: {
  link: Pick<ServerLink, "account" | "publicAddress" | "reachable">;
}) {
  if (link.account === "" || link.publicAddress === "") {
    return null;
  }
  return (
    <p className="text-sm text-pretty break-all text-muted-foreground">
      {link.reachable
        ? `Reachable from outside the LAN at ${link.publicAddress}.`
        : `The Hub couldn’t reach the Server at ${link.publicAddress} from outside the LAN. Check that your router forwards that port to the Server.`}
    </p>
  );
}
