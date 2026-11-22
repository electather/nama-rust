import { useRouteContext } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { DownloadRing } from "@/features/requests/components/download-ring";
import { FulfillerLink } from "@/features/requests/components/fulfiller-link";
import { Reason } from "@/features/requests/components/reason";
import { RequestStateIcon } from "@/features/requests/components/request-state-icon";
import { StoppedDownloads } from "@/features/requests/components/stopped-downloads";
import {
  aboutRequest,
  downloadsOf,
  failedBy,
  reasonOf,
} from "@/features/requests/lib/request-facts";
import type { FollowedShow, Request } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/**
 * A Request's or Followed Show's `title`, then its state's icon and what it is and who asked, and
 * why it was Declined or Failed. While it downloads, a ring in its state's place says how far, and
 * why any download stopped.
 */
export function RequestDetails({
  request,
  title,
}: {
  request: Request | FollowedShow;
  title: ReactNode;
}) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  const downloads = downloadsOf(request);
  const state =
    downloads.length === 0 ? (
      <RequestStateIcon state={request.state} />
    ) : (
      <DownloadRing downloads={downloads} />
    );
  return (
    <div className="flex min-w-0 flex-1 basis-56 flex-col gap-0.5">
      {title}
      <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
        {state}
        {aboutRequest(request, signedIn.user?.id)}
      </p>
      <StoppedDownloads downloads={downloads} />
      <Reason reason={reasonOf(request)} />
      <FulfillerLink
        pluginId={failedBy(request)}
        mayFix={may(signedIn.user, Permission.MANAGE_SETTINGS)}
      />
    </div>
  );
}
