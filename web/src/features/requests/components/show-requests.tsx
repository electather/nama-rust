import { RequestRow } from "@/features/requests/components/request-row";
import type { ShowGroup } from "@/features/requests/lib/group-shows";

/** A Show's title, with its Followed Shows and Season Requests listed under it. */
export function ShowRequests({ showTitle, followed, seasons }: Omit<ShowGroup, "showId">) {
  return (
    <li className="border-b pt-3 last:border-b-0">
      <p className="truncate font-medium">{showTitle}</p>
      <ul aria-label={showTitle} className="flex flex-col ps-4">
        {followed.map((followedShow) => (
          <RequestRow key={`followed-${followedShow.id}`} request={followedShow} />
        ))}
        {seasons.map((request) => (
          <RequestRow key={String(request.id)} request={request} />
        ))}
      </ul>
    </li>
  );
}
