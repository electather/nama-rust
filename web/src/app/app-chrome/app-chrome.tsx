import { NavLink } from "@/app/app-chrome/nav-link";
import { RequestsLink } from "@/app/app-chrome/requests-link";
import { SearchBox } from "@/app/app-chrome/search-box";
import { SignOutButton } from "@/app/app-chrome/sign-out-button";

/**
 * The bar above every signed-in page: where to go, searching your Libraries, and signing out.
 * Admin shows only when the User may open some Admin page, and the Pending count only to a Request
 * manager.
 */
export function AppChrome({
  hasAdminPages,
  managesRequests,
}: {
  hasAdminPages: boolean;
  managesRequests: boolean;
}) {
  return (
    // Lined up with the page's column: each link's own padding makes up the rest of the inset.
    <header className="mx-auto flex w-full max-w-5xl flex-wrap items-center px-1.5 py-3 sm:gap-1 sm:px-2">
      <nav aria-label="Main" className="flex items-center sm:gap-1">
        <NavLink to="/" activeOptions={{ exact: true }}>
          Libraries
        </NavLink>
        <RequestsLink managesRequests={managesRequests} />
        <NavLink to="/settings">Settings</NavLink>
        {hasAdminPages && <NavLink to="/admin">Admin</NavLink>}
      </nav>
      <SearchBox />
      <div className="ms-auto sm:ms-0">
        <SignOutButton />
      </div>
    </header>
  );
}
