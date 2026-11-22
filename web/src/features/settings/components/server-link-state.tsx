import { ServerLinkCode } from "@/features/settings/components/server-link-code";
import { ServerLinkRow } from "@/features/settings/components/server-link-row";
import type { ShownLink } from "@/features/settings/lib/server-link";

/** Where the Server's or a User's Link stands, with the one button that changes it: Link, Cancel or Unlink. */
export function ServerLinkState({
  link,
  enterUnder,
  pending,
  onStart,
  onUnlink,
}: {
  link: ShownLink;
  /** The Hub's form the code goes in, such as “Link a Server”. */
  enterUnder: string;
  pending: boolean;
  onStart: () => void;
  onUnlink: () => void;
}) {
  if (link.code !== "") {
    return (
      <ServerLinkCode link={link} enterUnder={enterUnder} pending={pending} onCancel={onUnlink} />
    );
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
