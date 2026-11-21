import { type ReactNode, Suspense, useState } from "react";

import { PaneFailed } from "@/features/settings/components/pane-failed";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PaneSkeleton } from "@/features/settings/components/pane-skeleton";
import { useRetryReads } from "@/features/settings/data/use-retry-reads";
import type { Section } from "@/features/settings/lib/sections";
import { ErrorBoundary } from "@/shared/components/error-boundary";

/**
 * A pane's settings while they load, and in their place, if they don't, what couldn't load and a
 * way to try again. The rest of Settings stays as it is either way.
 */
export function PaneBoundary({ section, children }: { section: Section; children: ReactNode }) {
  const [tries, setTries] = useState(0);
  const retry = useRetryReads();
  const failed = (
    <PaneFrame section={section}>
      <PaneFailed
        section={section}
        onRetry={() => {
          retry();
          setTries(tries + 1);
        }}
      />
    </PaneFrame>
  );
  const loading = (
    <PaneFrame section={section}>
      <PaneSkeleton />
    </PaneFrame>
  );
  return (
    // Keyed by the try, so trying again mounts a boundary that hasn't caught anything.
    <ErrorBoundary key={tries} failed={failed}>
      <Suspense fallback={loading}>{children}</Suspense>
    </ErrorBoundary>
  );
}
