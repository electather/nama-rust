import { ServerLinkCode } from "@/features/settings/components/server-link-code";
import { ServerLinkRow } from "@/features/settings/components/server-link-row";
import type { ServerLink } from "@/gen/nama/v1/server_pb";

/** Where Linking stands, with the one button that changes it: Link, Cancel or Unlink. */
export function ServerLinkState({
  link,
  pending,
  onStart,
  onUnlink,
}: {
  link: ServerLink;
  pending: boolean;
  onStart: () => void;
  onUnlink: () => void;
}) {
  if (link.code !== "") {
    return <ServerLinkCode link={link} pending={pending} onCancel={onUnlink} />;
  }
  return link.account === "" ? (
    <ServerLinkRow
      says={<span className="text-muted-foreground">Not Linked</span>}
      action="Link to the Hub"
      doing="Getting a code…"
      variant="secondary"
      pending={pending}
      onClick={onStart}
    />
  ) : (
    <ServerLinkRow
      says={
        <>
          Linked to <span className="font-medium">{link.account}</span>
        </>
      }
      action="Unlink"
      doing="Unlinking…"
      variant="ghost"
      pending={pending}
      onClick={onUnlink}
    />
  );
}
