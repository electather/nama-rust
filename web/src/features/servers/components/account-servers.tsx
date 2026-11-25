import { Suspense } from "react";

import { ServerList } from "@/features/servers/components/server-list";
import { ErrorBoundary } from "@/shared/components/error-boundary";
import { Notice } from "@/shared/components/sign-in-layout/notice";

const failed = (
  <Notice kind="error">
    The Hub couldn’t list your Servers just now. Reload the page to try again.
  </Notice>
);

/** Each of the Account's Servers, with its Libraries under its name. */
export function AccountServers() {
  return (
    <ErrorBoundary failed={failed}>
      <Suspense fallback={null}>
        <ServerList />
      </Suspense>
    </ErrorBoundary>
  );
}
