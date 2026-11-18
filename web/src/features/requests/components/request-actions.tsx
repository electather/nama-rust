import { Manage } from "@/features/requests/components/manage";
import { Withdraw } from "@/features/requests/components/withdraw";
import { canWithdraw, targetOf } from "@/features/requests/lib/request-target";
import type { FollowedShow, Request } from "@/gen/nama/v1/request_pb";

/**
 * What the signed-in User `you` may do to `request`, a Request or a Followed Show: withdraw from it
 * if it's their own and Pending, or their own Followed Show, and approve, decline, retry or cancel
 * it if they `manage` Requests.
 */
export function RequestActions({
  request,
  you,
  manage,
}: {
  request: Request | FollowedShow;
  you: bigint | undefined;
  manage: boolean;
}) {
  const target = targetOf(request);
  return (
    // Pulled back by the buttons' own padding, so the first one's words line up with the title.
    // Hidden when it has no buttons, so it adds no space to the row.
    <div className="-ms-2.5 mt-1 flex basis-full flex-wrap gap-1 empty:hidden">
      {canWithdraw(request, you) && <Withdraw {...target} />}
      {manage && <Manage state={request.state} {...target} />}
    </div>
  );
}
