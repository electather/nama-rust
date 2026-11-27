import { Suspense } from "react";

import { LinkedServerNotices } from "@/features/servers/components/linked-server-notices";
import { ErrorBoundary } from "@/shared/components/error-boundary";
import { Notice } from "@/shared/components/sign-in-layout/notice";

const failed = (
  <Notice kind="error">
    The Hub couldn’t list your Servers just now. Reload the page to try again.
  </Notice>
);

/** Over Home at the Hub, a notice for each of the Account's Servers it can't show, or none. */
export function ServerNotices() {
  return (
    <ErrorBoundary failed={failed}>
      <Suspense fallback={null}>
        <LinkedServerNotices />
      </Suspense>
    </ErrorBoundary>
  );
}
