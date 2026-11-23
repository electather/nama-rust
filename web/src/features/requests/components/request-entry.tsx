import { useRouteContext } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { RequestActions } from "@/features/requests/components/request-actions";
import { RequestDetails } from "@/features/requests/components/request-details";
import type { FollowedShow, Request } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/** A Request or Followed Show under its `title`, and what the signed-in User may do to it at the end. */
export function RequestEntry({
  request,
  title,
}: {
  request: Request | FollowedShow;
  title: ReactNode;
}) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return (
    // A manager's buttons wrap under the title once they leave it too little room.
    <div className="flex min-w-0 flex-1 flex-wrap items-start gap-x-2.5 gap-y-1">
      <RequestDetails request={request} title={title} />
      <RequestActions
        request={request}
        you={signedIn.user?.id}
        manage={may(signedIn.user, Permission.MANAGE_REQUESTS)}
      />
    </div>
  );
}
