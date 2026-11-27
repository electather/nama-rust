import type { Transport } from "@connectrpc/connect";
import type { LinkOptions } from "@tanstack/react-router";

import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";

/**
 * The Server a title is on and the transport that reaches it: at the Hub, one of the Account's
 * Linked Servers, or on a Server's own web app, none, as the page's own Server (ADR 0105).
 */
export interface TitleServer<Linked extends LinkedServer | undefined = LinkedServer | undefined> {
  server: Linked;
  transport: Transport;
}

/**
 * `link`, opening on the title's Server `on`: its `?server=` names the Linked Server, keeping the
 * link's own search params, and never the current page's.
 */
export function linkOn(link: LinkOptions, on: { server: Pick<LinkedServer, "id"> | undefined }) {
  const own = typeof link.search === "object" ? link.search : {};
  // Set even when undefined, so `retainSearchParams` doesn't keep another Server's (#2416).
  return { ...link, search: { ...own, server: on.server?.id } } satisfies LinkOptions;
}
