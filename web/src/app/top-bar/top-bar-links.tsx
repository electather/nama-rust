import { usePendingRequestCount } from "@/app/data/use-pending-request-count";
import { PendingBadge } from "@/app/top-bar/pending-badge";
import { TopBarLink } from "@/app/top-bar/top-bar-link";

/**
 * The top bar's links. Admin shows only when the User may open some Admin page, and the Pending
 * count only to a Request manager. A phone has no room for them: there the action bar's "Jump to"
 * goes to each.
 */
export function TopBarLinks({
  hasAdminPages,
  managesRequests,
}: {
  hasAdminPages: boolean;
  managesRequests: boolean;
}) {
  const pending = usePendingRequestCount(managesRequests);
  return (
    <nav aria-label="Main" className="hidden items-center gap-0.5 sm:flex">
      <TopBarLink to="/" activeOptions={{ exact: true }}>
        Home
      </TopBarLink>
      <TopBarLink
        to="/requests"
        aria-label={pending === 0 ? "Requests" : `Requests, ${pending} Pending`}
      >
        Requests
        {pending > 0 && <PendingBadge pending={pending} />}
      </TopBarLink>
      {hasAdminPages && <TopBarLink to="/admin">Admin</TopBarLink>}
    </nav>
  );
}
