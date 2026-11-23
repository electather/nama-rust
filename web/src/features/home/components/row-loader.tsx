import { type ReactNode, Suspense } from "react";

import { ErrorBoundary } from "@/shared/components/error-boundary";

/**
 * A row's shelf, `children`, once it loads, `loading` until then, or nothing if it fails: a
 * Plugin's failures belong in Settings' Plugins section, and a Library's on its own page.
 */
export function RowLoader({ loading, children }: { loading: ReactNode; children: ReactNode }) {
  return (
    <ErrorBoundary failed={null}>
      <Suspense fallback={loading}>{children}</Suspense>
    </ErrorBoundary>
  );
}
