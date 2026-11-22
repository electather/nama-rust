import { usePendingRequestCount } from "@/app/data/use-pending-request-count";
import { PendingBadge } from "@/app/top-bar/pending-badge";
import { TopBarLink } from "@/app/top-bar/top-bar-link";
import { type Section, sectionSearch } from "@/features/settings/lib/sections";

/**
 * The top bar's links. Admin, which opens Settings at `adminSection`, shows only when there is one,
 * and the Pending count only to a Request manager. A phone has no room for them: there the action
 * bar's "Jump to" goes to each.
 */
export function TopBarLinks({
  adminSection,
  managesRequests,
}: {
  adminSection: Section | undefined;
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
      {adminSection !== undefined && (
        // Settings opens over this page, in a history entry of its own.
        <TopBarLink to="." search={(rest) => ({ ...rest, ...sectionSearch(adminSection) })}>
          Admin
        </TopBarLink>
      )}
    </nav>
  );
}
