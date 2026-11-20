import { Suspense, useId } from "react";

import { ContinueWatchingFrame } from "@/features/home/components/continue-watching-frame";
import { ContinueWatchingShelf } from "@/features/home/components/continue-watching-shelf";
import { ShelfSkeleton } from "@/features/home/components/shelf-skeleton";
import { backdropTile } from "@/features/home/lib/shelf-tiles";
import { ErrorBoundary } from "@/shared/components/error-boundary";

/** Continue Watching, loading on its own, its skeleton until then, or that it failed. */
export function ContinueWatching() {
  const headingId = useId();
  const failed = (
    <ContinueWatchingFrame headingId={headingId}>
      <p className="text-muted-foreground">
        Couldn’t load Continue Watching. Check that the Server is running.
      </p>
    </ContinueWatchingFrame>
  );
  const loading = (
    <ContinueWatchingFrame headingId={headingId}>
      <ShelfSkeleton tile={backdropTile} aspect="aspect-4/3" />
    </ContinueWatchingFrame>
  );
  return (
    <ErrorBoundary failed={failed}>
      <Suspense fallback={loading}>
        <ContinueWatchingShelf headingId={headingId} />
      </Suspense>
    </ErrorBoundary>
  );
}
