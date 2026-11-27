import { Suspense } from "react";

import { ErrorBoundary } from "@/shared/components/error-boundary";
import { FolderTree } from "@/shared/components/folder-tree";

const NOTE = "p-3 text-sm text-muted-foreground";

const failed = <p className={NOTE}>Couldn’t list the Server’s folders. Check that it’s running.</p>;

/** The Server's folders as a tree once its top level loads, or that it couldn't. */
export function FolderTreeLoader({
  path,
  onPick,
}: {
  path: string;
  onPick: (path: string) => void;
}) {
  return (
    <ErrorBoundary failed={failed}>
      <Suspense fallback={<p className={NOTE}>Loading folders…</p>}>
        <FolderTree path={path} onPick={onPick} />
      </Suspense>
    </ErrorBoundary>
  );
}
