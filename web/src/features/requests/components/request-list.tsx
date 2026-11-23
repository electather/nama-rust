import { RequestRow } from "@/features/requests/components/request-row";
import { ShowRequests } from "@/features/requests/components/show-requests";
import { useRequests } from "@/features/requests/data/use-requests";
import { groupShows, keyOf } from "@/features/requests/lib/group-shows";
import { type StateFilter, filterName } from "@/features/requests/lib/state-filter";
import { ShowMore } from "@/shared/components/show-more";

/** The Requests and Followed Shows in the state `filter` names, a page at a time. */
export function RequestList({ label, filter }: { label: string; filter: StateFilter }) {
  const { data, hasNextPage, fetchNextPage } = useRequests(filter);
  const requests = data.pages.flatMap((page) => page.requests);
  const followedShows = data.pages.flatMap((page) => page.followedShows);
  if (requests.length + followedShows.length === 0) {
    return (
      <p className="py-3 text-sm text-muted-foreground">
        {filter === "all"
          ? "No Requests yet. Request a Movie or Show from its page."
          : `No ${filterName(filter)} Requests.`}
      </p>
    );
  }
  return (
    <>
      <ul aria-label={label} className="flex flex-col">
        {groupShows(requests, followedShows).map((entry) =>
          "seasons" in entry ? (
            <ShowRequests key={`show-${keyOf(entry)}`} {...entry} />
          ) : (
            // Only a Movie's Request is listed on its own: the rest go under their Show.
            entry.$typeName === "nama.v1.Request" && (
              <RequestRow key={String(entry.id)} request={entry} />
            )
          ),
        )}
      </ul>
      {hasNextPage && <ShowMore load={fetchNextPage} loaded={data.pages.length} />}
    </>
  );
}
