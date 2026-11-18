import { Brand } from "@/app/top-bar/brand";
import { RequestsLink } from "@/app/top-bar/requests-link";
import { TopBarLink } from "@/app/top-bar/top-bar-link";

/**
 * The mark and the main pages. Admin shows only when the User may open some Admin page, and the
 * Pending count only to a Request manager. Under 640px the links drop out, and the action bar's
 * Jump to takes their place.
 */
export function TopBarStart({
  hasAdminPages,
  managesRequests,
}: {
  hasAdminPages: boolean;
  managesRequests: boolean;
}) {
  return (
    <div className="flex min-w-0 items-center gap-2.5">
      <Brand />
      <span aria-hidden className="mx-1 hidden h-4 w-px shrink-0 bg-border sm:block" />
      <nav aria-label="Main" className="hidden items-center gap-0.5 sm:flex">
        <TopBarLink to="/" activeOptions={{ exact: true }}>
          Home
        </TopBarLink>
        <RequestsLink managesRequests={managesRequests} />
        {hasAdminPages && <TopBarLink to="/admin">Admin</TopBarLink>}
      </nav>
    </div>
  );
}
