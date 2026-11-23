import { type ReactElement, type ReactNode, Suspense, useState } from "react";

import { PaneFailed } from "@/features/settings/components/pane-failed";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PaneSkeleton } from "@/features/settings/components/pane-skeleton";
import { useRetryReads } from "@/features/settings/data/use-retry-reads";
import type { Section } from "@/features/settings/lib/sections";
import { ErrorBoundary } from "@/shared/components/error-boundary";

/**
 * A part of a pane that reads on its own: `rows` of skeleton while it loads, and if it doesn't,
 * that `what` couldn't load and a way to try again. The rest of the pane shows as it is either way.
 * For a whole `pane`, each sits in its frame.
 */
export function SectionBoundary({
  what,
  rows = 1,
  pane,
  children,
}: {
  what: string;
  rows?: number;
  pane?: Section;
  children: ReactNode;
}) {
  const around = (shown: ReactElement) =>
    pane === undefined ? shown : <PaneFrame section={pane}>{shown}</PaneFrame>;
  const [tries, setTries] = useState(0);
  const retry = useRetryReads();
  const failed = around(
    <PaneFailed
      what={what}
      onRetry={() => {
        retry();
        setTries(tries + 1);
      }}
    />,
  );
  return (
    // Keyed by the try, so trying again mounts a boundary that hasn't caught anything.
    <ErrorBoundary key={tries} failed={failed}>
      <Suspense fallback={around(<PaneSkeleton rows={rows} />)}>{children}</Suspense>
    </ErrorBoundary>
  );
}
