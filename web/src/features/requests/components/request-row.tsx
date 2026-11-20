import { useRouteContext } from "@tanstack/react-router";

import { FulfillerLink } from "@/features/requests/components/fulfiller-link";
import { Reason } from "@/features/requests/components/reason";
import { RequestActions } from "@/features/requests/components/request-actions";
import { RequestProgress } from "@/features/requests/components/request-progress";
import { RequestStateLabel } from "@/features/requests/components/request-state-label";
import { aboutRequest, failedBy, reasonOf, titleOf } from "@/features/requests/lib/request-facts";
import type { FollowedShow, Request } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/** A Request, or a Followed Show, with its state and what the signed-in User may do to it. */
export function RequestRow({ request }: { request: Request | FollowedShow }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return (
    // Wraps so the downloads and the actions get lines of their own, the full width of the row.
    <li className="flex min-h-16 flex-wrap items-center gap-x-3 border-b py-3 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{titleOf(request)}</p>
        <p className="text-sm text-muted-foreground">{aboutRequest(request, signedIn.user?.id)}</p>
        <Reason reason={reasonOf(request)} />
        <FulfillerLink
          pluginId={failedBy(request)}
          mayFix={may(signedIn.user, Permission.MANAGE_SETTINGS)}
        />
      </div>
      <RequestStateLabel state={request.state} />
      {request.$typeName === "nama.v1.Request" && <RequestProgress request={request} />}
      <RequestActions
        request={request}
        you={signedIn.user?.id}
        manage={may(signedIn.user, Permission.MANAGE_REQUESTS)}
      />
    </li>
  );
}
