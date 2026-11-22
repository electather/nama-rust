import { type ReactNode, Suspense } from "react";

import { RowShelf } from "@/features/home/components/row-shelf";
import type { PluginHomeRow } from "@/gen/nama/v1/browse_pb";
import { ErrorBoundary } from "@/shared/components/error-boundary";

/**
 * A Plugin's row once it loads, `loading` until then, or nothing if it fails: a Plugin's failures
 * belong in Settings' Plugins section.
 */
export function RowLoader({
  row,
  headingId,
  loading,
}: {
  row: PluginHomeRow;
  headingId: string;
  loading: ReactNode;
}) {
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={loading}>
        <RowShelf row={row} headingId={headingId} />
      </Suspense>
    </ErrorBoundary>
  );
}
