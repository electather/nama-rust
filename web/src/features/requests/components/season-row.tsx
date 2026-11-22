import { RequestEntry } from "@/features/requests/components/request-entry";
import { titleOf } from "@/features/requests/lib/request-facts";
import type { FollowedShow, Request } from "@/gen/nama/v1/request_pb";

/** A Season Request, or a Followed Show, under its Show's title. */
export function SeasonRow({ request }: { request: Request | FollowedShow }) {
  return (
    <li className="flex">
      <RequestEntry
        request={request}
        title={<p className="truncate text-sm">{titleOf(request)}</p>}
      />
    </li>
  );
}
