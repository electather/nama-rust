import { Link } from "@tanstack/react-router";

import { SeasonRow } from "@/features/requests/components/season-row";
import { useTitleLinkSearch } from "@/features/requests/hooks/use-title-link-search";
import type { ShowGroup } from "@/features/requests/lib/group-shows";
import { titleParam } from "@/shared/lib/title-param";

/** A Show's title, a link to its page, with its Followed Shows and Season Requests listed under it. */
export function ShowRequests({ showId, showTitle, followed, seasons }: ShowGroup) {
  const search = useTitleLinkSearch();
  return (
    <li className="flex flex-col gap-1.5 py-2.5 not-first:border-t">
      <Link
        to="/shows/$showId"
        search={search}
        params={{ showId: titleParam(showId) }}
        className="truncate text-sm font-medium underline-offset-4 hover:underline"
      >
        {showTitle}
      </Link>
      <ul aria-label={showTitle} className="flex flex-col gap-2">
        {followed.map((followedShow) => (
          <SeasonRow key={`followed-${followedShow.id}`} request={followedShow} />
        ))}
        {seasons.map((request) => (
          <SeasonRow key={String(request.id)} request={request} />
        ))}
      </ul>
    </li>
  );
}
