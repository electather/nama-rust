// fallow-ignore-next-line policy-violation:feature-layout/data-hooks -- no query is built here: a title at the Hub reads its art's widths from, and acts on, its own Server, as `_member/_server` hands a Server's pages theirs
import { TransportProvider } from "@connectrpc/connect-query";
import type { ReactNode } from "react";

import type { TitleServer } from "@/shared/lib/title-server";

/** `children`, a title's art and actions, run against its Server `on`. */
export function OnServer({ on, children }: { on: TitleServer; children: ReactNode }) {
  return <TransportProvider transport={on.transport}>{children}</TransportProvider>;
}
