import { ServerLinkCode } from "@/features/settings/components/server-link-code";
import { ServerLinkRow } from "@/features/settings/components/server-link-row";
import { ServerNotLinked } from "@/features/settings/components/server-not-linked";
import type { ShownLink } from "@/features/settings/lib/server-link";

/**
 * Where the Server's or a User's Link stands, with the button that changes it: Link, Cancel or
 * Unlink. Given `onRedirect`, Link opens the Hub's approval, and a code is the other way.
 */
export function ServerLinkState({
  link,
  enterUnder,
  pending,
  onStart,
  onRedirect,
  onUnlink,
}: {
  link: ShownLink;
  /** The Hub's form the code goes in, such as “Link a Server”. */
  enterUnder: string;
  pending: boolean;
  /** Starts Linking by a code. */
  onStart: () => void;
  /** Starts Linking by redirect to the Hub's approval. */
  onRedirect: (() => void) | undefined;
  onUnlink: () => void;
}) {
  if (link.code !== "") {
    return (
      <ServerLinkCode link={link} enterUnder={enterUnder} pending={pending} onCancel={onUnlink} />
    );
  }
  return link.account === "" ? (
    <ServerNotLinked pending={pending} onStart={onStart} onRedirect={onRedirect} />
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
