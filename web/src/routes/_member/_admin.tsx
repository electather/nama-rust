import { Code, ConnectError } from "@connectrpc/connect";
import { Link, Outlet, createFileRoute } from "@tanstack/react-router";

import { buttonVariants } from "@/components/ui/button";
import { adminPages, adminPagesFor } from "@/lib/access";

/**
 * Admin pages, each for Users with the Permission `adminPages` gives it. Anyone else gets the same
 * page the Server's own refusal shows.
 */
export const Route = createFileRoute("/_member/_admin")({
  beforeLoad: ({ context: { signedIn }, location }) => {
    const allowed = adminPagesFor(signedIn.user);
    // ponytail: matched by path prefix, since no page's path starts another's. `/admin` itself
    // only needs some page.
    const page = adminPages.find(({ to }) => location.pathname.startsWith(to));
    if (allowed.length === 0 || (page && !allowed.includes(page))) {
      throw new ConnectError("this page needs a Permission you don't have", Code.PermissionDenied);
    }
    return { adminPages: allowed };
  },
  component: AdminLayout,
});

function AdminLayout() {
  const { adminPages: allowed } = Route.useRouteContext();
  return (
    <>
      {/* Lined up with the page's column: each link's own padding makes up the rest of the inset. */}
      <nav aria-label="Admin" className="mx-auto flex w-full max-w-5xl flex-wrap gap-1 sm:px-2">
        {allowed.map(({ to, label }) => (
          <Link
            key={to}
            to={to}
            className={buttonVariants({
              variant: "ghost",
              className: "h-8 aria-[current=page]:bg-muted aria-[current=page]:text-foreground",
            })}
          >
            {label}
          </Link>
        ))}
      </nav>
      <Outlet />
    </>
  );
}
