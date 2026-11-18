import { NavLink } from "@/app/app-chrome/nav-link";
import { PendingBadge } from "@/app/app-chrome/pending-badge";
import { usePendingRequestCount } from "@/app/data/use-pending-request-count";

/**
 * The link to Requests, with how many are Pending for a Request manager. Request Events refetch the
 * count along with every other Request query.
 */
export function RequestsLink({ managesRequests }: { managesRequests: boolean }) {
  const pending = usePendingRequestCount(managesRequests);
  if (pending === 0) {
    return <NavLink to="/requests">Requests</NavLink>;
  }
  return (
    <NavLink to="/requests" aria-label={`Requests, ${pending} Pending`}>
      Requests
      <PendingBadge pending={pending} />
    </NavLink>
  );
}
