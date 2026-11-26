// fallow-ignore-next-line policy-violation:feature-layout/data-hooks -- no query is built here: a tile at the Hub reads its art's widths from, and acts on, its own Server, as `_member/_server` hands a Server's pages theirs
import { TransportProvider } from "@connectrpc/connect-query";
import type { ReactNode } from "react";

import type { ReachedServer } from "@/features/home/lib/hub-rows";

/** A tile at the Hub, `children`, run against the Server `transport` reaches. */
export function OnServer({
  transport,
  children,
}: Pick<ReachedServer, "transport"> & { children: ReactNode }) {
  return <TransportProvider transport={transport}>{children}</TransportProvider>;
}
