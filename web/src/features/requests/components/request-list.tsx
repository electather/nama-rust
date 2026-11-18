import { RequestRow } from "@/features/requests/components/request-row";
import { ShowRequests } from "@/features/requests/components/show-requests";
import { StateFilterMenu } from "@/features/requests/components/state-filter-menu";
import { useRequests } from "@/features/requests/data/use-requests";
import { groupShows, keyOf } from "@/features/requests/lib/group-shows";
import { type StateFilter, filterName } from "@/features/requests/lib/state-filter";
import { ShowMore } from "@/shared/components/show-more";

/**
 * The Requests and Followed Shows in the state `filter` names, a page at a time, under the menu
 * that chooses it.
 */
export function RequestList({
  filter,
  choose,
}: {
  filter: StateFilter;
  choose: (state: StateFilter) => void;
}) {
  const { data, hasNextPage, fetchNextPage } = useRequests(filter);
  const requests = data.pages.flatMap((page) => page.requests);
  const followedShows = data.pages.flatMap((page) => page.followedShows);
  return (
    <>
      <div className="-ms-2.5 mt-8">
        <StateFilterMenu chosen={filter} choose={choose} />
      </div>
      {requests.length + followedShows.length === 0 ? (
        <p className="mt-4 text-muted-foreground">
          {filter === "all" ? "No Requests yet." : `No ${filterName(filter)} Requests.`}
        </p>
      ) : (
        <ul className="mt-4 flex max-w-2xl flex-col">
          {groupShows(requests, followedShows).map((entry) =>
            "seasons" in entry ? (
              <ShowRequests key={`show-${keyOf(entry)}`} {...entry} />
            ) : (
              <RequestRow key={String(entry.id)} request={entry} />
            ),
          )}
        </ul>
      )}
      {hasNextPage && <ShowMore load={fetchNextPage} loaded={data.pages.length} />}
    </>
  );
}
