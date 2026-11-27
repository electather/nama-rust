import { Suspense } from "react";

import { ReachNotice } from "@/features/servers/components/reach-notice";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { ErrorBoundary } from "@/shared/components/error-boundary";
import { Notice } from "@/shared/components/sign-in-layout/notice";

/** Why Home can't show `server`, such as its being offline; none while reaching it or once shown. */
export function ServerNotice({ server }: { server: LinkedServer }) {
  return (
    <ErrorBoundary failed={<Notice kind="info">{server.name} is offline.</Notice>}>
      <Suspense fallback={null}>
        <ReachNotice server={server} />
      </Suspense>
    </ErrorBoundary>
  );
}
