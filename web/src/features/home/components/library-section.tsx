import { Suspense } from "react";

import { LibraryShelf } from "@/features/home/components/library-shelf";
import { ErrorBoundary } from "@/shared/components/error-boundary";

/** Home's Libraries, or nothing if they fail, so the rest of Home still shows. */
export function LibrarySection() {
  return (
    <ErrorBoundary failed={null}>
      {/* Home's loader already waited on the Libraries, so this shows only while a failed load is tried again. */}
      <Suspense fallback={null}>
        <LibraryShelf />
      </Suspense>
    </ErrorBoundary>
  );
}
