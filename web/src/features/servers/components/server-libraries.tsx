import { CloudOff } from "lucide-react";
import { Suspense } from "react";

import { ServerContents } from "@/features/servers/components/server-contents";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import { ErrorBoundary } from "@/shared/components/error-boundary";
import { StateLabel } from "@/shared/components/state-label";

const connecting = <p className="text-sm text-muted-foreground">Connecting…</p>;
const offline = <StateLabel label="Offline" Icon={CloudOff} />;

/** `server`'s Libraries, or why they can't show, such as its being offline. */
export function ServerLibraries({ server }: { server: LinkedServer }) {
  return (
    <ErrorBoundary failed={offline}>
      <Suspense fallback={connecting}>
        <ServerContents server={server} />
      </Suspense>
    </ErrorBoundary>
  );
}
