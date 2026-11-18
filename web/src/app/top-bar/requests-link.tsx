import { usePendingRequestCount } from "@/app/data/use-pending-request-count";
import { PendingBadge } from "@/app/top-bar/pending-badge";
import { TopBarLink } from "@/app/top-bar/top-bar-link";

/**
 * The link to Requests, with how many are Pending for a Request manager. Request Events refetch the
 * count along with every other Request query.
 */
export function RequestsLink({ managesRequests }: { managesRequests: boolean }) {
  const pending = usePendingRequestCount(managesRequests);
  if (pending === 0) {
    return <TopBarLink to="/requests">Requests</TopBarLink>;
  }
  return (
    <TopBarLink to="/requests" aria-label={`Requests, ${pending} Pending`}>
      Requests
      <PendingBadge pending={pending} />
    </TopBarLink>
  );
}
